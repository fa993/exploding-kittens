use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CatBreed {
    #[serde(rename = "Beard Cat")]
    Beard,
    #[serde(rename = "Cattermelon")]
    Cattermelon,
    #[serde(rename = "Hairy Potato Cat")]
    HairyPotato,
    #[serde(rename = "Rainbow-Ralphing Cat")]
    RainbowRalphing,
    #[serde(rename = "Tacocat")]
    Tacocat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum CardType {
    Defuse,
    Nope,
    #[serde(rename = "Exploding Kitten")]
    ExplodingKitten,
    Attack,
    Skip,
    Favor,
    Shuffle,
    #[serde(rename = "See the Future")]
    SeeTheFuture,
    #[serde(rename = "cat")]
    Cat(CatBreed),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Card {
    pub id: String,
    pub kind: CardType,
}

impl Card {
    pub fn new(kind: CardType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            kind,
        }
    }
}

impl CardType {
    /// Generates the raw list of cards for a standard deck (before setup/deal)
    /// Total: 56 Cards
    pub fn standard_deck_distribution() -> Vec<CardType> {
        let mut deck = Vec::new();

        // 1. Core Actions
        for _ in 0..5 {
            deck.push(CardType::Defuse);
        }
        for _ in 0..5 {
            deck.push(CardType::Nope);
        }
        for _ in 0..6 {
            deck.push(CardType::ExplodingKitten);
        }
        for _ in 0..4 {
            deck.push(CardType::Attack);
        }
        for _ in 0..4 {
            deck.push(CardType::Skip);
        }
        for _ in 0..4 {
            deck.push(CardType::Favor);
        }
        for _ in 0..4 {
            deck.push(CardType::Shuffle);
        }
        for _ in 0..5 {
            deck.push(CardType::SeeTheFuture);
        }

        // 2. Cat Cards (4 of each breed)
        let breeds = [
            CatBreed::Beard,
            CatBreed::Cattermelon,
            CatBreed::HairyPotato,
            CatBreed::RainbowRalphing,
            CatBreed::Tacocat,
        ];

        for breed in breeds {
            for _ in 0..4 {
                deck.push(CardType::Cat(breed));
            }
        }

        deck
    }
}
