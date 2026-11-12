use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Canonical ECO opening classification returned by [`classify_opening`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpeningInfo {
    pub eco: String,
    pub name: String,
    pub family: String,
}

#[derive(Debug, Clone)]
pub struct OpeningEntry {
    pub eco: String,
    pub name: String,
    pub family: String,
    pub uci_line: Vec<String>,
}

struct StaticOpeningEntry {
    eco: &'static str,
    name: &'static str,
    family: &'static str,
    uci_line: &'static [&'static str],
}

/// Curated canonical ECO opening book (A00-E99) using UCI move prefixes.
///
/// The longest matching prefix wins, so specific variations (e.g. Sicilian Najdorf)
/// automatically override broader families (e.g. Sicilian Defense).
const CANONICAL_OPENINGS: &[StaticOpeningEntry] = &[
    // --- A: Flank Openings & Benoni ---
    StaticOpeningEntry { eco: "A00", name: "Polish Opening", family: "Flank", uci_line: &["b2b4"] },
    StaticOpeningEntry { eco: "A01", name: "Nimzo-Larsen Attack", family: "Flank", uci_line: &["b2b3"] },
    StaticOpeningEntry { eco: "A02", name: "Bird's Opening", family: "Bird", uci_line: &["f2f4"] },
    StaticOpeningEntry { eco: "A04", name: "Reti Opening", family: "Reti", uci_line: &["g1f3"] },
    StaticOpeningEntry { eco: "A06", name: "Reti Opening: King's Indian Attack", family: "Reti", uci_line: &["g1f3", "d7d5", "g2g3"] },
    StaticOpeningEntry { eco: "A09", name: "Reti Gambit", family: "Reti", uci_line: &["g1f3", "d7d5", "c2c4"] },
    StaticOpeningEntry { eco: "A10", name: "English Opening", family: "English", uci_line: &["c2c4"] },
    StaticOpeningEntry { eco: "A13", name: "English Opening: Agincourt Defense", family: "English", uci_line: &["c2c4", "e7e6"] },
    StaticOpeningEntry { eco: "A15", name: "English Opening: Anglo-Indian", family: "English", uci_line: &["c2c4", "g8f6"] },
    StaticOpeningEntry { eco: "A20", name: "English Opening: Reversed Sicilian", family: "English", uci_line: &["c2c4", "e7e5"] },
    StaticOpeningEntry { eco: "A40", name: "Queen's Pawn Game", family: "Queen's Pawn", uci_line: &["d2d4"] },
    StaticOpeningEntry { eco: "A45", name: "Trompowsky Attack", family: "Queen's Pawn", uci_line: &["d2d4", "g8f6", "c1g5"] },
    StaticOpeningEntry { eco: "A52", name: "Budapest Gambit", family: "Budapest", uci_line: &["d2d4", "g8f6", "c2c4", "e7e5"] },
    StaticOpeningEntry { eco: "A57", name: "Benko Gambit", family: "Benoni", uci_line: &["d2d4", "g8f6", "c2c4", "c7c5", "d4d5", "b7b5"] },
    StaticOpeningEntry { eco: "A60", name: "Modern Benoni", family: "Benoni", uci_line: &["d2d4", "g8f6", "c2c4", "c7c5", "d4d5", "e7e6"] },
    StaticOpeningEntry { eco: "A80", name: "Dutch Defense", family: "Dutch", uci_line: &["d2d4", "f7f5"] },
    StaticOpeningEntry { eco: "A87", name: "Dutch Defense: Leningrad", family: "Dutch", uci_line: &["d2d4", "f7f5", "g2g3", "g8f6", "f1g2", "g7g6"] },

    // --- B: Semi-Open Games (Sicilian, Caro-Kann, French, Pirc, Alekhine) ---
    StaticOpeningEntry { eco: "B00", name: "King's Pawn Opening", family: "King's Pawn", uci_line: &["e2e4"] },
    StaticOpeningEntry { eco: "B01", name: "Scandinavian Defense", family: "Scandinavian", uci_line: &["e2e4", "d7d5"] },
    StaticOpeningEntry { eco: "B02", name: "Alekhine's Defense", family: "Alekhine", uci_line: &["e2e4", "g8f6"] },
    StaticOpeningEntry { eco: "B06", name: "Modern Defense", family: "Modern", uci_line: &["e2e4", "g7g6"] },
    StaticOpeningEntry { eco: "B07", name: "Pirc Defense", family: "Pirc", uci_line: &["e2e4", "d7d6", "d2d4", "g8f6"] },
    StaticOpeningEntry { eco: "B10", name: "Caro-Kann Defense", family: "Caro-Kann", uci_line: &["e2e4", "c7c6"] },
    StaticOpeningEntry { eco: "B12", name: "Caro-Kann: Advance Variation", family: "Caro-Kann", uci_line: &["e2e4", "c7c6", "d2d4", "d7d5", "e4e5"] },
    StaticOpeningEntry { eco: "B13", name: "Caro-Kann: Exchange Variation", family: "Caro-Kann", uci_line: &["e2e4", "c7c6", "d2d4", "d7d5", "e4d5", "c6d5"] },
    StaticOpeningEntry { eco: "B18", name: "Caro-Kann: Classical Variation", family: "Caro-Kann", uci_line: &["e2e4", "c7c6", "d2d4", "d7d5", "b1c3", "d5e4", "c3e4", "c8f5"] },
    StaticOpeningEntry { eco: "B20", name: "Sicilian Defense", family: "Sicilian", uci_line: &["e2e4", "c7c5"] },
    StaticOpeningEntry { eco: "B21", name: "Sicilian: Smith-Morra Gambit", family: "Sicilian", uci_line: &["e2e4", "c7c5", "d2d4", "c5d4", "c2c3"] },
    StaticOpeningEntry { eco: "B22", name: "Sicilian: Alapin Variation", family: "Sicilian", uci_line: &["e2e4", "c7c5", "c2c3"] },
    StaticOpeningEntry { eco: "B23", name: "Sicilian: Closed Variation", family: "Sicilian", uci_line: &["e2e4", "c7c5", "b1c3"] },
    StaticOpeningEntry { eco: "B30", name: "Sicilian: Rossolimo", family: "Sicilian", uci_line: &["e2e4", "c7c5", "g1f3", "b8c6", "f1b5"] },
    StaticOpeningEntry { eco: "B50", name: "Sicilian Defense: Open", family: "Sicilian", uci_line: &["e2e4", "c7c5", "g1f3", "d7d6"] },
    StaticOpeningEntry { eco: "B70", name: "Sicilian: Dragon Variation", family: "Sicilian", uci_line: &["e2e4", "c7c5", "g1f3", "d7d6", "d2d4", "c5d4", "f3d4", "g8f6", "b1c3", "g7g6"] },
    StaticOpeningEntry { eco: "B90", name: "Sicilian: Najdorf Variation", family: "Sicilian", uci_line: &["e2e4", "c7c5", "g1f3", "d7d6", "d2d4", "c5d4", "f3d4", "g8f6", "b1c3", "a7a6"] },

    // --- C: Open Games & French Defense ---
    StaticOpeningEntry { eco: "C00", name: "French Defense", family: "French", uci_line: &["e2e4", "e7e6"] },
    StaticOpeningEntry { eco: "C02", name: "French Defense: Advance Variation", family: "French", uci_line: &["e2e4", "e7e6", "d2d4", "d7d5", "e4e5"] },
    StaticOpeningEntry { eco: "C11", name: "French Defense: Classical", family: "French", uci_line: &["e2e4", "e7e6", "d2d4", "d7d5", "b1c3", "g8f6"] },
    StaticOpeningEntry { eco: "C15", name: "French Defense: Winawer", family: "French", uci_line: &["e2e4", "e7e6", "d2d4", "d7d5", "b1c3", "f8b4"] },
    StaticOpeningEntry { eco: "C20", name: "King's Pawn Game", family: "Open Game", uci_line: &["e2e4", "e7e5"] },
    StaticOpeningEntry { eco: "C23", name: "Bishop's Opening", family: "Open Game", uci_line: &["e2e4", "e7e5", "f1c4"] },
    StaticOpeningEntry { eco: "C25", name: "Vienna Game", family: "Vienna", uci_line: &["e2e4", "e7e5", "b1c3"] },
    StaticOpeningEntry { eco: "C30", name: "King's Gambit", family: "King's Gambit", uci_line: &["e2e4", "e7e5", "f2f4"] },
    StaticOpeningEntry { eco: "C41", name: "Philidor Defense", family: "Philidor", uci_line: &["e2e4", "e7e5", "g1f3", "d7d6"] },
    StaticOpeningEntry { eco: "C42", name: "Petrov's Defense", family: "Petrov", uci_line: &["e2e4", "e7e5", "g1f3", "g8f6"] },
    StaticOpeningEntry { eco: "C44", name: "Scotch Game", family: "Scotch", uci_line: &["e2e4", "e7e5", "g1f3", "b8c6", "d2d4"] },
    StaticOpeningEntry { eco: "C46", name: "Three Knights Opening", family: "Open Game", uci_line: &["e2e4", "e7e5", "g1f3", "b8c6", "b1c3"] },
    StaticOpeningEntry { eco: "C47", name: "Four Knights Game", family: "Four Knights", uci_line: &["e2e4", "e7e5", "g1f3", "b8c6", "b1c3", "g8f6"] },
    StaticOpeningEntry { eco: "C50", name: "Italian Game", family: "Italian", uci_line: &["e2e4", "e7e5", "g1f3", "b8c6", "f1c4"] },
    StaticOpeningEntry { eco: "C51", name: "Italian Game: Evans Gambit", family: "Italian", uci_line: &["e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "f8c5", "b2b4"] },
    StaticOpeningEntry { eco: "C55", name: "Italian Game: Two Knights Defense", family: "Italian", uci_line: &["e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "g8f6"] },
    StaticOpeningEntry { eco: "C60", name: "Ruy Lopez", family: "Ruy Lopez", uci_line: &["e2e4", "e7e5", "g1f3", "b8c6", "f1b5"] },
    StaticOpeningEntry { eco: "C65", name: "Ruy Lopez: Berlin Defense", family: "Ruy Lopez", uci_line: &["e2e4", "e7e5", "g1f3", "b8c6", "f1b5", "g8f6"] },
    StaticOpeningEntry { eco: "C68", name: "Ruy Lopez: Exchange Variation", family: "Ruy Lopez", uci_line: &["e2e4", "e7e5", "g1f3", "b8c6", "f1b5", "a7a6", "b5c6"] },
    StaticOpeningEntry { eco: "C84", name: "Ruy Lopez: Closed", family: "Ruy Lopez", uci_line: &["e2e4", "e7e5", "g1f3", "b8c6", "f1b5", "a7a6", "b5a4", "g8f6", "e1g1", "f8e7"] },

    // --- D: Closed & Semi-Closed Queen's Pawn Games ---
    StaticOpeningEntry { eco: "D00", name: "Queen's Pawn: Mason / London System", family: "London", uci_line: &["d2d4", "d7d5", "c1f4"] },
    StaticOpeningEntry { eco: "D02", name: "London System", family: "London", uci_line: &["d2d4", "d7d5", "g1f3", "g8f6", "c1f4"] },
    StaticOpeningEntry { eco: "D06", name: "Queen's Gambit", family: "Queen's Gambit", uci_line: &["d2d4", "d7d5", "c2c4"] },
    StaticOpeningEntry { eco: "D07", name: "Queen's Gambit: Chigorin Defense", family: "Queen's Gambit", uci_line: &["d2d4", "d7d5", "c2c4", "b8c6"] },
    StaticOpeningEntry { eco: "D10", name: "Slav Defense", family: "Slav", uci_line: &["d2d4", "d7d5", "c2c4", "c7c6"] },
    StaticOpeningEntry { eco: "D20", name: "Queen's Gambit Accepted", family: "Queen's Gambit", uci_line: &["d2d4", "d7d5", "c2c4", "d5c4"] },
    StaticOpeningEntry { eco: "D30", name: "Queen's Gambit Declined", family: "Queen's Gambit", uci_line: &["d2d4", "d7d5", "c2c4", "e7e6"] },
    StaticOpeningEntry { eco: "D35", name: "Queen's Gambit Declined: Exchange", family: "Queen's Gambit", uci_line: &["d2d4", "d7d5", "c2c4", "e7e6", "b1c3", "g8f6", "c4d5", "e6d5"] },
    StaticOpeningEntry { eco: "D80", name: "Grunfeld Defense", family: "Grunfeld", uci_line: &["d2d4", "g8f6", "c2c4", "g7g6", "b1c3", "d7d5"] },

    // --- E: Indian Defenses ---
    StaticOpeningEntry { eco: "E00", name: "Catalan Opening", family: "Catalan", uci_line: &["d2d4", "g8f6", "c2c4", "e7e6", "g2g3"] },
    StaticOpeningEntry { eco: "E12", name: "Queen's Indian Defense", family: "Queen's Indian", uci_line: &["d2d4", "g8f6", "c2c4", "e7e6", "g1f3", "b7b6"] },
    StaticOpeningEntry { eco: "E20", name: "Nimzo-Indian Defense", family: "Nimzo-Indian", uci_line: &["d2d4", "g8f6", "c2c4", "e7e6", "b1c3", "f8b4"] },
    StaticOpeningEntry { eco: "E60", name: "King's Indian Defense", family: "King's Indian", uci_line: &["d2d4", "g8f6", "c2c4", "g7g6"] },
    StaticOpeningEntry { eco: "E70", name: "King's Indian: Classical / Normal", family: "King's Indian", uci_line: &["d2d4", "g8f6", "c2c4", "g7g6", "b1c3", "f8g7", "e2e4", "d7d6"] },
];

/// Prefix-trie opening book allowing fast classification from either the built-in
/// canonical lines or an extended TSV dataset (e.g. Lichess CC0 `chess-openings`).
#[derive(Debug, Default, Clone)]
pub struct OpeningBook {
    root: TrieNode,
}

#[derive(Debug, Default, Clone)]
struct TrieNode {
    info: Option<OpeningInfo>,
    children: HashMap<String, TrieNode>,
}

impl OpeningBook {
    pub fn canonical() -> Self {
        let mut book = Self::default();
        for entry in CANONICAL_OPENINGS {
            book.insert(
                entry.uci_line.iter().copied(),
                OpeningInfo {
                    eco: entry.eco.to_string(),
                    name: entry.name.to_string(),
                    family: entry.family.to_string(),
                },
            );
        }
        book
    }

    /// Parses lines formatted as `eco\tname\tfamily\tuci1 uci2 ...` (skipping blank/comment lines).
    pub fn load_tsv(&mut self, tsv: &str) {
        for line in tsv.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("eco\t") {
                continue;
            }
            let mut cols = trimmed.split('\t');
            let (Some(eco), Some(name), Some(family), Some(moves)) =
                (cols.next(), cols.next(), cols.next(), cols.next())
            else {
                continue;
            };
            let uci_moves: Vec<&str> = moves.split_whitespace().collect();
            if !uci_moves.is_empty() {
                self.insert(
                    uci_moves.into_iter(),
                    OpeningInfo {
                        eco: eco.trim().to_string(),
                        name: name.trim().to_string(),
                        family: family.trim().to_string(),
                    },
                );
            }
        }
    }

    pub fn insert<'a>(&mut self, uci_line: impl Iterator<Item = &'a str>, info: OpeningInfo) {
        let mut node = &mut self.root;
        for uci in uci_line {
            node = node.children.entry(uci.to_string()).or_default();
        }
        node.info = Some(info);
    }

    pub fn classify(&self, uci_moves: &[String]) -> Option<OpeningInfo> {
        let mut node = &self.root;
        let mut best: Option<OpeningInfo> = None;
        for mv in uci_moves {
            let Some(next) = node.children.get(mv) else {
                break;
            };
            node = next;
            if let Some(info) = &node.info {
                best = Some(info.clone());
            }
        }
        best
    }
}

/// Classifies a sequence of UCI move strings (e.g. `["e2e4", "c7c5", ...]`)
/// into the deepest matching canonical ECO opening.
pub fn classify_opening(uci_moves: &[String]) -> Option<OpeningInfo> {
    OpeningBook::canonical().classify(uci_moves)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_longest_matching_opening() {
        let moves: Vec<String> = [
            "e2e4", "c7c5", "g1f3", "d7d6", "d2d4", "c5d4", "f3d4", "g8f6", "b1c3", "a7a6",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let info = classify_opening(&moves).expect("should match Najdorf");
        assert_eq!(info.eco, "B90");
        assert_eq!(info.family, "Sicilian");
        assert!(info.name.contains("Najdorf"));
    }

    #[test]
    fn tsv_loader_extends_opening_book() {
        let mut book = OpeningBook::canonical();
        book.load_tsv("C52\tItalian: Evans Accepted\tItalian\te2e4 e7e5 g1f3 b8c6 f1c4 f8c5 b2b4 c5b4\n");
        let moves: Vec<String> = [
            "e2e4", "e7e5", "g1f3", "b8c6", "f1c4", "f8c5", "b2b4", "c5b4",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let info = book.classify(&moves).unwrap();
        assert_eq!(info.eco, "C52");
        assert_eq!(info.name, "Italian: Evans Accepted");
    }

    #[test]
    fn returns_none_on_empty_game() {
        assert!(classify_opening(&[]).is_none());
    }
}
