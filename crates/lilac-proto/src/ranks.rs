use serde::{Deserialize, Serialize};

/// Valorant-inspired competitive tier ladder.
///
/// Players below [`RankTier::Gold`] compete purely in the 0-100 RR Cohort ladder.
/// Reaching [`RankTier::Gold`] and completing the qualification threshold unlocks
/// an actual public global rating (`global_rating`) and global leaderboard placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RankTier {
    Iron,
    Bronze,
    Silver,
    Gold,
    Platinum,
    Diamond,
    Ascendant,
    Immortal,
    Radiant,
}

impl RankTier {
    pub const DEFAULT_GOLD_QUALIFICATION_GAMES: i32 = 10;

    /// Returns `true` for tiers eligible to unlock/display an actual global rating.
    pub fn is_gold_or_above(self) -> bool {
        self >= Self::Gold
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Iron => "iron",
            Self::Bronze => "bronze",
            Self::Silver => "silver",
            Self::Gold => "gold",
            Self::Platinum => "platinum",
            Self::Diamond => "diamond",
            Self::Ascendant => "ascendant",
            Self::Immortal => "immortal",
            Self::Radiant => "radiant",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "iron" => Some(Self::Iron),
            "bronze" => Some(Self::Bronze),
            "silver" => Some(Self::Silver),
            "gold" => Some(Self::Gold),
            "platinum" => Some(Self::Platinum),
            "diamond" => Some(Self::Diamond),
            "ascendant" => Some(Self::Ascendant),
            "immortal" => Some(Self::Immortal),
            "radiant" => Some(Self::Radiant),
            _ => None,
        }
    }
}

/// Sub-division within a [`RankTier`] (`I` is lowest, `III` is highest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RankDivision {
    I = 1,
    II = 2,
    III = 3,
}

impl RankDivision {
    pub fn from_i16(val: i16) -> Self {
        match val {
            2 => Self::II,
            3 => Self::III,
            _ => Self::I,
        }
    }

    pub fn as_i16(self) -> i16 {
        self as i16
    }
}

/// Complete competitive rank snapshot for a given time control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RankBadge {
    pub tier: RankTier,
    pub division: RankDivision,
    /// Valorant-style 0..=100 Rank Rating inside the current division (or uncapped in Radiant).
    pub rr_points: i32,
    pub demotion_shield: bool,
    pub peak_tier: RankTier,
    pub peak_division: RankDivision,
    /// Becomes `true` once the player reaches Gold+ and clears `gold_qualification_target` games.
    pub global_rating_qualified: bool,
    /// Populated only when `global_rating_qualified == true`.
    pub global_rating: Option<i32>,
    pub gold_qualification_games: i32,
    pub gold_qualification_target: i32,
}

impl RankBadge {
    pub fn format_label(&self) -> String {
        let div = match self.tier {
            RankTier::Radiant => "",
            _ => match self.division {
                RankDivision::I => " I",
                RankDivision::II => " II",
                RankDivision::III => " III",
            },
        };
        if let Some(rating) = self.global_rating.filter(|_| self.global_rating_qualified) {
            format!(
                "{}{} • {} RR (Global {})",
                self.tier.as_str().to_uppercase(),
                div,
                self.rr_points,
                rating
            )
        } else {
            format!(
                "{}{} • {}/100 RR",
                self.tier.as_str().to_uppercase(),
                div,
                self.rr_points
            )
        }
    }
}

/// Supported matchmaking & rating time-control buckets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeControlKind {
    Bullet,
    Blitz,
    Rapid,
    Classical,
}

impl TimeControlKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bullet => "bullet",
            Self::Blitz => "blitz",
            Self::Rapid => "rapid",
            Self::Classical => "classical",
        }
    }

    pub fn default_clocks_ms(self) -> (u64, u64) {
        match self {
            Self::Bullet => (60_000, 1_000),
            Self::Blitz => (180_000, 2_000),
            Self::Rapid => (600_000, 5_000),
            Self::Classical => (1_800_000, 15_000),
        }
    }
}

/// Itemized Valorant-style RR breakdown computed after a ranked match (Task 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RrBreakdown {
    pub base_delta: i32,
    pub nemesis_bonus: i32,
    pub performance_bonus: i32,
    pub net_delta: i32,
    pub tier_before: RankTier,
    pub division_before: RankDivision,
    pub rr_before: i32,
    pub tier_after: RankTier,
    pub division_after: RankDivision,
    pub rr_after: i32,
    pub promoted: bool,
    pub demoted: bool,
    pub unlocked_global_rating: Option<i32>,
}

/// Complete game termination reason (including clock/network outcomes beyond board rules).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum GameTermination {
    Checkmate { winner: chess_core::Side },
    Stalemate,
    InsufficientMaterial,
    FiftyMoveRule,
    ThreefoldRepetition,
    Timeout { winner: chess_core::Side },
    Resigned { winner: chess_core::Side },
    DrawByAgreement,
    Aborted,
    Abandoned { winner: chess_core::Side },
}

