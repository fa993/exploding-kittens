#[cfg(test)]
mod tests {
    use crate::game::cards::{Card, CardType};
    use crate::game::engine::{GameContext, GameEvent, GamePhase};

    // ========================================================================
    // TEST HELPERS
    // ========================================================================

    fn create_running_game() -> GameContext {
        let mut game = GameContext::new();
        // Use fixed IDs for easier testing
        game.add_player("id_alice".to_string(), "Alice".to_string())
            .unwrap();
        game.add_player("id_bob".to_string(), "Bob".to_string())
            .unwrap();

        // Start game (StartGame doesn't strictly check actor_id in our logic, so we pass "system")
        game.transition(GameEvent::StartGame, "system").unwrap();

        // Clear hands for deterministic testing
        for p in &mut game.players {
            p.hand.clear();
        }
        game
    }

    fn give_card(game: &mut GameContext, player_idx: usize, card_type: CardType) -> usize {
        let card = Card::new(card_type);
        game.players[player_idx].hand.push(card);
        game.players[player_idx].hand.len() - 1
    }

    fn force_top_card(game: &mut GameContext, card_type: CardType) {
        let card = Card::new(card_type);
        game.deck.push(card);
    }

    // Helper to get the current player's ID cleanly (avoids borrow checker issues)
    fn get_current_player_id(game: &GameContext) -> String {
        game.players[game.current_player_idx].id.clone()
    }

    // ========================================================================
    // THE TESTS
    // ========================================================================

    #[test]
    fn test_game_setup_and_join() {
        let mut game = GameContext::new();

        game.add_player("id_1".into(), "Alice".into()).unwrap();

        // Cannot start with 1 player
        let err = game.transition(GameEvent::StartGame, "id_1");
        assert!(err.is_err());

        game.add_player("id_2".into(), "Bob".into()).unwrap();

        // Should start now
        game.transition(GameEvent::StartGame, "id_1").unwrap();

        assert!(matches!(game.phase, GamePhase::PlayerTurn));
        assert_eq!(game.current_player_idx, 0);
        assert_eq!(game.actions_remaining, 1);
    }

    #[test]
    fn test_safe_draw_pass_turn() {
        let mut game = create_running_game();
        force_top_card(&mut game, CardType::Nope);

        let p1_id = get_current_player_id(&game);
        let p1_idx = game.current_player_idx;

        // Action: Draw (Pass correct ID)
        game.transition(GameEvent::DrawCard, &p1_id).unwrap();

        // Assert: Turn passed
        assert_ne!(game.current_player_idx, p1_idx);
        assert_eq!(game.actions_remaining, 1);
    }

    #[test]
    fn test_security_wrong_turn() {
        let mut game = create_running_game();

        // It is currently Alice's turn (index 0)
        assert_eq!(game.current_player_idx, 0);

        // Bob (index 1) tries to draw
        let bob_id = "id_bob".to_string();
        let res = game.transition(GameEvent::DrawCard, &bob_id);

        // Assert: Request rejected
        assert!(res.is_err());
        assert_eq!(res.unwrap_err(), "Not your turn!");
    }

    #[test]
    fn test_exploding_kitten_draw() {
        let mut game = create_running_game();
        force_top_card(&mut game, CardType::ExplodingKitten);

        let pid = get_current_player_id(&game);

        // Action: Draw
        game.transition(GameEvent::DrawCard, &pid).unwrap();

        // Assert: State is ExplosionPending
        match game.phase {
            GamePhase::ExplosionPending { timer_seconds } => assert_eq!(timer_seconds, 30),
            _ => panic!("Should be in ExplosionPending phase"),
        }
    }

    #[test]
    fn test_defuse_mechanic() {
        let mut game = create_running_game();
        let pid = get_current_player_id(&game);
        let p_idx = game.current_player_idx;

        // 1. Trigger Bomb
        force_top_card(&mut game, CardType::ExplodingKitten);
        game.transition(GameEvent::DrawCard, &pid).unwrap();

        // 2. Give Player a Defuse
        let card_idx = give_card(&mut game, p_idx, CardType::Defuse);

        // Action: Play Defuse (using same pid)
        game.transition(
            GameEvent::PlayDefuse {
                card_idx,
                insert_depth: 0,
            },
            &pid,
        )
        .unwrap();

        // Assert: Safe again
        assert!(matches!(game.phase, GamePhase::PlayerTurn));

        // Turn should have passed
        assert_ne!(game.current_player_idx, p_idx);
    }

    #[test]
    fn test_attack_card() {
        let mut game = create_running_game();
        let attacker_idx = game.current_player_idx;
        let attacker_id = get_current_player_id(&game);

        let idx = give_card(&mut game, attacker_idx, CardType::Attack);

        // Action: Attack
        game.transition(GameEvent::PlayAttack { card_idx: idx }, &attacker_id)
            .unwrap();

        // Assert: Turn passed immediately
        assert_ne!(game.current_player_idx, attacker_idx);
        // Assert: Victim has 2 actions
        assert_eq!(game.actions_remaining, 2);
    }

    #[test]
    fn test_see_the_future() {
        let mut game = create_running_game();
        let pid = get_current_player_id(&game);

        force_top_card(&mut game, CardType::Skip);

        let idx = give_card(&mut game, 0, CardType::SeeTheFuture);

        // Action
        game.transition(GameEvent::PlaySeeTheFuture { card_idx: idx }, &pid)
            .unwrap();

        // Assert: Result is stored
        assert!(game.last_action_result.is_some());
    }

    #[test]
    fn test_illegal_moves_logic() {
        let mut game = create_running_game();
        let pid = get_current_player_id(&game);

        // 1. Wrong Phase (Try to play attack during explosion)
        force_top_card(&mut game, CardType::ExplodingKitten);
        game.transition(GameEvent::DrawCard, &pid).unwrap(); // Now in ExplosionPending

        // --- FIX IS HERE ---
        // We must copy the index into a variable BEFORE borrowing game mutably
        let current_player_idx = game.current_player_idx;
        let idx = give_card(&mut game, current_player_idx, CardType::Attack);
        // -------------------

        // Use the ID of the person currently exploding
        let res = game.transition(GameEvent::PlayAttack { card_idx: idx }, &pid);
        assert!(res.is_err());
    }
}
