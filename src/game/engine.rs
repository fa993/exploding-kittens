use crate::game::cards::{Card, CardType};
use rand::prelude::IteratorRandom;
use rand::seq::SliceRandom;
use rand::thread_rng;
use serde::{Deserialize, Serialize};

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

    // Audit Log (Client reads this to know what happened)
    pub logs: Vec<GameLog>,

    // Transient UI Helpers (Sent to client, cleared next turn)
    pub last_action_result: Option<String>, // e.g. JSON of "See Future" cards
}

/// The Inputs
#[derive(Debug, Clone, Deserialize)]
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
    TimerExpired, // Frontend tells us animation finished / time ran out
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
        // 1. SECURITY CHECK: Is it actually this player's turn?
        // We skip this check for 'StartGame' since no one is "current" yet.
        if !matches!(event, GameEvent::StartGame) {
            let current_player_id = &self.players[self.current_player_idx].id;

            if current_player_id != actor_id {
                return Err("Not your turn!".to_string());
            }
        }

        // We clone phase to avoid borrow checker fighting during the match
        let current_phase = self.phase.clone();

        match (current_phase, event) {
            // ----------------------------------------------------------------
            // PHASE: WAITING
            // ----------------------------------------------------------------
            (GamePhase::WaitingForPlayers, GameEvent::StartGame) => {
                if self.players.len() < 2 {
                    return Err("Need 2+ players".into());
                }
                self.setup_game();
                self.phase = GamePhase::PlayerTurn;
                self.log("Game Started!".to_string());
                Ok(())
            }

            // ----------------------------------------------------------------
            // PHASE: PLAYER TURN
            // ----------------------------------------------------------------
            (GamePhase::PlayerTurn, GameEvent::DrawCard) => {
                // 1. Peek top card
                let card = self.deck.pop().ok_or("Deck empty!")?;

                if matches!(card.kind, CardType::ExplodingKitten) {
                    self.log(format!(
                        "{} drew an Exploding Kitten!",
                        self.current_player_name()
                    ));
                    self.phase = GamePhase::ExplosionPending { timer_seconds: 30 };
                } else {
                    self.players[self.current_player_idx].hand.push(card);
                    self.log(format!(
                        "{} drew a card safely.",
                        self.current_player_name()
                    ));

                    // Decrement actions (handles Attack stacking)
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

                // End turn immediately, next player gets 2 turns
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

                let mut rng = thread_rng();
                self.deck.shuffle(&mut rng);
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

                // We store this in the context so the frontend can render the modal
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

                // Discard both (sort desc to remove safely)
                let mut sorted = card_indices.clone();
                sorted.sort_by(|a, b| b.cmp(a));
                for idx in sorted {
                    self.discard_card(idx);
                }

                let stolen = self.steal_random_card(target_idx);
                if let Some(c) = stolen {
                    self.log(format!(
                        "{} played a Pair and stole a card. {:#?}",
                        self.current_player_name(),
                        c.kind
                    ));
                }
                Ok(())
            }

            // ----------------------------------------------------------------
            // PHASE: EXPLOSION PENDING
            // ----------------------------------------------------------------
            (
                GamePhase::ExplosionPending { .. },
                GameEvent::PlayDefuse {
                    card_idx,
                    insert_depth,
                },
            ) => {
                self.validate_card(card_idx, CardType::Defuse)?;
                self.discard_card(card_idx);

                // Re-insert Kitten
                let kitten = Card::new(CardType::ExplodingKitten);
                let safe_depth = std::cmp::min(insert_depth, self.deck.len());
                let index = self.deck.len() - safe_depth;
                self.deck.insert(index, kitten);

                self.log(format!(
                    "{} defused the kitten!",
                    self.current_player_name()
                ));

                // Turn ends after successful defuse
                self.actions_remaining = self.actions_remaining.saturating_sub(1);
                if self.actions_remaining == 0 {
                    self.next_turn(1);
                }
                self.phase = GamePhase::PlayerTurn; // Return to normal
                Ok(())
            }

            (GamePhase::ExplosionPending { .. }, GameEvent::TimerExpired) => {
                self.log(format!("BOOM! {} exploded.", self.current_player_name()));
                self.players[self.current_player_idx].is_eliminated = true;

                // Check Win Condition
                let survivors: Vec<usize> = self
                    .players
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| !p.is_eliminated)
                    .map(|(i, _)| i)
                    .collect();

                if survivors.len() == 1 {
                    self.phase = GamePhase::GameOver {
                        winner_idx: survivors[0],
                    };
                    self.log(format!(
                        "Game Over! {} wins!",
                        self.players[survivors[0]].name
                    ));
                } else {
                    self.next_turn(1);
                    self.phase = GamePhase::PlayerTurn;
                }
                Ok(())
            }

            // ----------------------------------------------------------------
            // FALLBACK / INVALID
            // ----------------------------------------------------------------
            (GamePhase::GameOver { .. }, _) => Err("Game is over".into()),
            _ => Err("Invalid action for current phase".into()),
        }
    }

    // ========================================================================
    // HELPERS
    // ========================================================================

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

        // Deal 4 cards + 1 Defuse
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

        // Add Kittens + Remaining Defuses to deck
        let needed_kittens = self.players.len() - 1;
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
            } // Sanity break
        }
        self.current_player_idx = idx;
        self.actions_remaining = actions;
        self.last_action_result = None; // Clear transient UI data
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

        let mut rng = thread_rng();
        let idx = (0..target.hand.len()).choose(&mut rng).unwrap();
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
        // Simple timestamp mockup. In real app use SystemTime
        let ts = 0;
        self.logs.push(GameLog {
            timestamp: ts,
            message: msg,
        });
    }

    /// Creates a sanitized view for a specific player (hiding others' hands/deck)
    pub fn get_view_for_player(&self, player_id: &str) -> GameContext {
        let mut view = self.clone();

        // Hide Deck
        view.deck = vec![]; // Or obscure them

        // Hide other players' hands
        for p in &mut view.players {
            if p.id != player_id {
                // Keep the COUNT of cards, but remove the data
                // In a real app you might replace them with "CardBack" placeholders
                // For this struct, we just empty it or you'd need a ViewStruct.
                // We'll leave it empty for safety.
                p.hand = vec![];
            }
        }

        // We DO keep the logs, actions_remaining, etc.
        view
    }
}
