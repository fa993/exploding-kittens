use crate::game::cards::{Card, CardType};
use rand::Rng; // For gen_range
use rand::prelude::IteratorRandom;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

// ============================================================================
// 1. DATA STRUCTURES
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GamePhase {
    WaitingForPlayers,
    PlayerTurn,
    ExplosionPending { timer_seconds: u8 },
    GameOver { winner_idx: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub hand: Vec<Card>,
    pub is_eliminated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameLog {
    pub timestamp: u64,
    pub message: String,
}

/// The Single Source of Truth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameContext {
    pub phase: GamePhase,
    pub deck: Vec<Card>,
    pub discard_pile: Vec<Card>,
    pub players: Vec<Player>,
    pub current_player_idx: usize,

    // Mechanics
    pub actions_remaining: u8, // 1 = Normal, 2+ = Attacked

    // NEW: Timestamp of the last valid move (Server Time)
    pub last_move_ts: u64,

    // Audit Log (Client reads this to know what happened)
    pub logs: Vec<GameLog>,

    // Transient UI Helpers (Sent to client, cleared next turn)
    pub last_action_result: Option<String>,
}

/// The Inputs
#[derive(Debug, Clone, Deserialize, PartialEq)] // Added PartialEq for checks
#[serde(tag = "event", content = "data")]
pub enum GameEvent {
    StartGame,
    DrawCard,
    PlayAttack {
        card_idx: usize,
    },
    PlaySkip {
        card_idx: usize,
    },
    PlayShuffle {
        card_idx: usize,
    },
    PlaySeeTheFuture {
        card_idx: usize,
    },
    PlayFavor {
        card_idx: usize,
        target_idx: usize,
    },
    PlayPair {
        card_indices: Vec<usize>,
        target_idx: usize,
    },
    PlayDefuse {
        card_idx: usize,
        insert_depth: usize,
    },
    TimerExpired, // Backend automated event
}

#[derive(Serialize)]
pub struct GameView {
    pub phase: GamePhase,
    pub deck_count: usize,
    pub discard_pile: Vec<Card>,
    pub players: Vec<PlayerView>,
    pub current_player_idx: usize,
    pub my_hand: Vec<Card>,
    pub logs: Vec<GameLog>,
    pub last_action_result: Option<String>,
    // NEW: Send timestamp to frontend
    pub last_move_ts: u64,
}

#[derive(Serialize)]
pub struct PlayerView {
    pub id: String,
    pub name: String,
    pub is_eliminated: bool,
    pub hand_count: usize,
}

// ============================================================================
// 2. THE BIG LOGIC
// ============================================================================

impl GameContext {
    pub fn new() -> Self {
        Self {
            phase: GamePhase::WaitingForPlayers,
            deck: Vec::new(),
            discard_pile: Vec::new(),
            players: Vec::new(),
            current_player_idx: 0,
            actions_remaining: 0,
            last_move_ts: 0,
            logs: Vec::new(),
            last_action_result: None,
        }
    }

    pub fn add_player(&mut self, id: String, name: String) -> Result<(), String> {
        if !matches!(self.phase, GamePhase::WaitingForPlayers) {
            return Err("Game already started".to_string());
        }
        if self.players.len() >= 5 {
            return Err("Lobby full".to_string());
        }
        self.players.push(Player {
            id,
            name,
            hand: Vec::new(),
            is_eliminated: false,
        });
        self.log(format!("Player joined. Total: {}", self.players.len()));
        Ok(())
    }

    /// THE GIANT STATE MACHINE FUNCTION
    pub fn transition(&mut self, event: GameEvent, actor_id: &str) -> Result<(), String> {
        // 1. SECURITY CHECK
        // Allow "system" to bypass turn checks (for automated timers)
        if actor_id != "system" && !matches!(event, GameEvent::StartGame) {
            let current_player_id = &self.players[self.current_player_idx].id;
            if current_player_id != actor_id {
                return Err("Not your turn!".to_string());
            }
        }

        let current_phase = self.phase.clone();

        let result = match (current_phase, event) {
            // --- START ---
            (GamePhase::WaitingForPlayers, GameEvent::StartGame) => {
                if self.players.len() < 2 {
                    return Err("Need 2+ players".into());
                }
                self.setup_game();
                self.phase = GamePhase::PlayerTurn;
                self.log("Game Started!".to_string());
                Ok(())
            }

            // --- NORMAL ACTIONS ---
            (GamePhase::PlayerTurn, GameEvent::DrawCard) => {
                let card = self.deck.pop().ok_or("Deck empty!")?;

                if matches!(card.kind, CardType::ExplodingKitten) {
                    self.log(format!(
                        "{} drew an Exploding Kitten!",
                        self.current_player_name()
                    ));
                    self.phase = GamePhase::ExplosionPending { timer_seconds: 30 };
                } else {
                    self.log(format!(
                        "{} drew a card safely. {:?}",
                        self.current_player_name(),
                        card.kind,
                    ));
                    self.players[self.current_player_idx].hand.push(card);
                    self.actions_remaining = self.actions_remaining.saturating_sub(1);
                    if self.actions_remaining == 0 {
                        self.next_turn(1);
                    }
                }
                Ok(())
            }

            (GamePhase::PlayerTurn, GameEvent::PlayAttack { card_idx }) => {
                self.validate_card(card_idx, CardType::Attack)?;
                self.discard_card(card_idx);
                self.log(format!("{} attacked!", self.current_player_name()));
                self.next_turn(2);
                Ok(())
            }

            (GamePhase::PlayerTurn, GameEvent::PlaySkip { card_idx }) => {
                self.validate_card(card_idx, CardType::Skip)?;
                self.discard_card(card_idx);
                self.log(format!("{} played Skip.", self.current_player_name()));
                self.actions_remaining = self.actions_remaining.saturating_sub(1);
                if self.actions_remaining == 0 {
                    self.next_turn(1);
                }
                Ok(())
            }

            (GamePhase::PlayerTurn, GameEvent::PlayShuffle { card_idx }) => {
                self.validate_card(card_idx, CardType::Shuffle)?;
                self.discard_card(card_idx);
                self.log(format!("{} shuffled the deck.", self.current_player_name()));
                self.deck.shuffle(&mut thread_rng());
                Ok(())
            }

            (GamePhase::PlayerTurn, GameEvent::PlaySeeTheFuture { card_idx }) => {
                self.validate_card(card_idx, CardType::SeeTheFuture)?;
                self.discard_card(card_idx);
                let count = std::cmp::min(3, self.deck.len());
                let peek: Vec<String> = self
                    .deck
                    .iter()
                    .rev()
                    .take(count)
                    .map(|c| format!("{:?}", c.kind))
                    .collect();
                self.last_action_result = Some(serde_json::to_string(&peek).unwrap());
                self.log(format!(
                    "{} gazed into the future.",
                    self.current_player_name()
                ));
                Ok(())
            }

            (
                GamePhase::PlayerTurn,
                GameEvent::PlayFavor {
                    card_idx,
                    target_idx,
                },
            ) => {
                self.validate_card(card_idx, CardType::Favor)?;
                if target_idx == self.current_player_idx {
                    return Err("Cannot favor self".into());
                }

                self.discard_card(card_idx);
                let stolen = self.steal_random_card(target_idx);
                if let Some(c) = stolen {
                    self.log(format!(
                        "{} asked for a Favor and got {:?}.",
                        self.current_player_name(),
                        c.kind
                    ));
                }
                Ok(())
            }

            (
                GamePhase::PlayerTurn,
                GameEvent::PlayPair {
                    card_indices,
                    target_idx,
                },
            ) => {
                if card_indices.len() != 2 {
                    return Err("Need 2 cards".into());
                }
                let c1 = &self.players[self.current_player_idx].hand[card_indices[0]];
                let c2 = &self.players[self.current_player_idx].hand[card_indices[1]];
                if c1.kind != c2.kind {
                    return Err("Cards must match".into());
                }

                let mut sorted = card_indices.clone();
                sorted.sort_by(|a, b| b.cmp(a));
                for idx in sorted {
                    self.discard_card(idx);
                }

                if let Some(c) = self.steal_random_card(target_idx) {
                    self.log(format!(
                        "{} played a Pair and stole a card. {:?}",
                        self.current_player_name(),
                        c.kind
                    ));
                }
                Ok(())
            }

            // --- BOMB DEFUSAL ---
            (
                GamePhase::ExplosionPending { .. },
                GameEvent::PlayDefuse {
                    card_idx,
                    insert_depth,
                },
            ) => {
                self.validate_card(card_idx, CardType::Defuse)?;
                self.discard_card(card_idx);

                let kitten = Card::new(CardType::ExplodingKitten);
                let safe_depth = std::cmp::min(insert_depth, self.deck.len());
                let index = self.deck.len() - safe_depth;
                self.deck.insert(index, kitten);

                self.log(format!(
                    "{} defused the kitten!",
                    self.current_player_name()
                ));

                self.actions_remaining = self.actions_remaining.saturating_sub(1);
                if self.actions_remaining == 0 {
                    self.next_turn(1);
                }
                self.phase = GamePhase::PlayerTurn;
                Ok(())
            }

            // --- AUTOMATED TIMEOUT (AFK HANDLER) ---
            (phase_state, GameEvent::TimerExpired) => {
                let p_idx = self.current_player_idx;

                // Define logic closure to avoid borrow issues
                let handle_explosion =
                    |ctx: &mut GameContext, player_idx: usize| -> Result<(), String> {
                        let player_name = ctx.players[player_idx].name.clone();
                        let defuse_pos = ctx.players[player_idx]
                            .hand
                            .iter()
                            .position(|c| matches!(c.kind, CardType::Defuse));

                        if let Some(defuse_idx) = defuse_pos {
                            // A. AUTO-DEFUSE
                            let card = ctx.players[player_idx].hand.remove(defuse_idx);
                            ctx.discard_pile.push(card);

                            let kitten = Card::new(CardType::ExplodingKitten);
                            let depth = thread_rng().gen_range(0..=ctx.deck.len());
                            ctx.deck.insert(depth, kitten);

                            ctx.log(format!(
                                "😅 Auto-pilot: {} used a Defuse to survive!",
                                player_name
                            ));

                            ctx.actions_remaining = ctx.actions_remaining.saturating_sub(1);
                            if ctx.actions_remaining == 0 {
                                ctx.next_turn(1);
                            }
                            ctx.phase = GamePhase::PlayerTurn;
                        } else {
                            // B. ELIMINATE
                            ctx.players[player_idx].is_eliminated = true;
                            ctx.log(format!("☠️ Timeout! {} exploded.", player_name));

                            let survivors: Vec<usize> = ctx
                                .players
                                .iter()
                                .enumerate()
                                .filter(|(_, p)| !p.is_eliminated)
                                .map(|(i, _)| i)
                                .collect();

                            if survivors.len() == 1 {
                                let winner = ctx.players[survivors[0]].name.clone();
                                ctx.phase = GamePhase::GameOver {
                                    winner_idx: survivors[0],
                                };
                                ctx.log(format!("Game Over! {} wins!", winner));
                            } else {
                                ctx.next_turn(1);
                                ctx.phase = GamePhase::PlayerTurn;
                            }
                        }
                        Ok(())
                    };

                match phase_state {
                    GamePhase::PlayerTurn => {
                        let name = self.players[p_idx].name.clone();
                        self.log(format!("💤 {} is asleep. Forcing draw...", name));

                        let card = self.deck.pop().ok_or("Deck empty")?;
                        if matches!(card.kind, CardType::ExplodingKitten) {
                            handle_explosion(self, p_idx)
                        } else {
                            self.log(format!("Auto-drew {:?} (Safe).", card.kind));
                            self.players[p_idx].hand.push(card);
                            self.actions_remaining = self.actions_remaining.saturating_sub(1);
                            if self.actions_remaining == 0 {
                                self.next_turn(1);
                            }
                            Ok(())
                        }
                    }
                    GamePhase::ExplosionPending { .. } => handle_explosion(self, p_idx),
                    _ => Ok(()),
                }
            }

            (GamePhase::GameOver { .. }, _) => Err("Game is over".into()),
            _ => Err("Invalid action for current phase".into()),
        };

        if result.is_ok() {
            // 2. UPDATE TIMESTAMP (Start of the "clock" for the next move)
            let start = SystemTime::now();
            let since_epoch = start.duration_since(UNIX_EPOCH).unwrap();
            self.last_move_ts = since_epoch.as_millis() as u64;
        }
        result
    }

    // --- HELPERS ---

    fn setup_game(&mut self) {
        let mut rng = thread_rng();
        let all_cards = CardType::standard_deck_distribution();

        let mut bulk = Vec::new();
        let mut kittens = Vec::new();
        let mut defuses = Vec::new();

        for k in all_cards {
            let c = Card::new(k.clone());
            match k {
                CardType::ExplodingKitten => kittens.push(c),
                CardType::Defuse => defuses.push(c),
                _ => bulk.push(c),
            }
        }
        bulk.shuffle(&mut rng);

        for p in &mut self.players {
            if let Some(d) = defuses.pop() {
                p.hand.push(d);
            }
            for _ in 0..4 {
                if let Some(c) = bulk.pop() {
                    p.hand.push(c);
                }
            }
        }

        let needed_kittens = self.players.len().saturating_sub(1);
        for _ in 0..needed_kittens {
            if let Some(k) = kittens.pop() {
                bulk.push(k);
            }
        }
        while let Some(d) = defuses.pop() {
            bulk.push(d);
        }

        bulk.shuffle(&mut rng);
        self.deck = bulk;
        self.current_player_idx = 0;
        self.actions_remaining = 1;
    }

    fn next_turn(&mut self, actions: u8) {
        let mut idx = (self.current_player_idx + 1) % self.players.len();
        let mut attempts = 0;
        while self.players[idx].is_eliminated {
            idx = (idx + 1) % self.players.len();
            attempts += 1;
            if attempts > 6 {
                break;
            }
        }
        self.current_player_idx = idx;
        self.actions_remaining = actions;
        self.last_action_result = None;
        self.log(format!("It is now {}'s turn.", self.current_player_name()));
    }

    fn validate_card(&self, idx: usize, expected: CardType) -> Result<(), String> {
        let p = &self.players[self.current_player_idx];
        if idx >= p.hand.len() {
            return Err("Index out of bounds".into());
        }
        if p.hand[idx].kind != expected {
            return Err("Wrong card type".into());
        }
        Ok(())
    }

    fn discard_card(&mut self, idx: usize) {
        let card = self.players[self.current_player_idx].hand.remove(idx);
        self.discard_pile.push(card);
    }

    fn steal_random_card(&mut self, target_idx: usize) -> Option<Card> {
        let target = &mut self.players[target_idx];
        if target.hand.is_empty() {
            return None;
        }
        let idx = (0..target.hand.len()).choose(&mut thread_rng()).unwrap();
        let card = target.hand.remove(idx);
        self.players[self.current_player_idx]
            .hand
            .push(card.clone());
        Some(card)
    }

    fn current_player_name(&self) -> String {
        self.players[self.current_player_idx].name.clone()
    }

    fn log(&mut self, msg: String) {
        let start = SystemTime::now();
        let since = start.duration_since(UNIX_EPOCH).unwrap();
        self.logs.push(GameLog {
            timestamp: since.as_millis() as u64,
            message: msg,
        });
    }

    pub fn get_view_for_player(&self, player_id: &str) -> GameView {
        let players_view = self
            .players
            .iter()
            .map(|p| PlayerView {
                id: p.id.clone(),
                name: p.name.clone(),
                is_eliminated: p.is_eliminated,
                hand_count: p.hand.len(),
            })
            .collect();

        let my_hand = self
            .players
            .iter()
            .find(|p| p.id == player_id)
            .map(|p| p.hand.clone())
            .unwrap_or_default();

        GameView {
            phase: self.phase.clone(),
            deck_count: self.deck.len(),
            discard_pile: self.discard_pile.clone(),
            players: players_view,
            current_player_idx: self.current_player_idx,
            my_hand,
            logs: self.logs.clone(),
            last_action_result: self.last_action_result.clone(),
            last_move_ts: self.last_move_ts,
        }
    }
}
