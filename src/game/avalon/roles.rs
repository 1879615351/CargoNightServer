use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alignment {
    Good,
    Evil,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Merlin,
    Percival,
    LoyalServant,
    Assassin,
    Minion,
    Morgana,
    Mordred,
    Oberon,
}

impl Role {
    pub fn name(&self) -> &str {
        match self {
            Role::Merlin => "梅林",
            Role::Percival => "派西维尔",
            Role::LoyalServant => "忠臣",
            Role::Assassin => "刺客",
            Role::Minion => "爪牙",
            Role::Morgana => "莫甘娜",
            Role::Mordred => "莫德雷德",
            Role::Oberon => "奥伯伦",
        }
    }

    pub fn alignment(&self) -> Alignment {
        match self {
            Role::Merlin | Role::Percival | Role::LoyalServant => Alignment::Good,
            Role::Assassin | Role::Minion | Role::Morgana | Role::Mordred | Role::Oberon => Alignment::Evil,
        }
    }

    pub fn is_evil_visible_to(&self, viewer: Role) -> bool {
        match (viewer, self) {
            (Role::Merlin, Role::Mordred) => false,
            (Role::Merlin, Role::Oberon) => false,
            (Role::Merlin, _) if self.alignment() == Alignment::Evil => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleConfig {
    pub roles: Vec<Role>,
}

pub fn get_recommended_roles(player_count: usize) -> Vec<Role> {
    match player_count {
        5 => vec![Role::Merlin, Role::LoyalServant, Role::LoyalServant, Role::Assassin, Role::Minion],
        6 => vec![Role::Merlin, Role::Percival, Role::LoyalServant, Role::LoyalServant, Role::Assassin, Role::Minion],
        7 => vec![Role::Merlin, Role::Percival, Role::LoyalServant, Role::LoyalServant, Role::Assassin, Role::Morgana, Role::Minion],
        8 => vec![Role::Merlin, Role::Percival, Role::LoyalServant, Role::LoyalServant, Role::LoyalServant, Role::Assassin, Role::Morgana, Role::Minion],
        9 => vec![Role::Merlin, Role::Percival, Role::LoyalServant, Role::LoyalServant, Role::LoyalServant, Role::LoyalServant, Role::Assassin, Role::Morgana, Role::Mordred],
        10 => vec![Role::Merlin, Role::Percival, Role::LoyalServant, Role::LoyalServant, Role::LoyalServant, Role::LoyalServant, Role::Assassin, Role::Morgana, Role::Mordred, Role::Oberon],
        _ => vec![Role::Merlin, Role::Percival, Role::LoyalServant, Role::LoyalServant, Role::Assassin, Role::Minion],
    }
}

pub fn get_mission_sizes(player_count: usize) -> Vec<usize> {
    match player_count {
        5 => vec![2, 3, 2, 3, 3],
        6 => vec![2, 3, 4, 3, 4],
        7 => vec![2, 3, 3, 4, 4],
        _ => vec![3, 4, 4, 5, 5],
    }
}

pub fn fails_needed(mission: usize, player_count: usize) -> usize {
    if mission == 3 && player_count >= 7 {
        2
    } else {
        1
    }
}
