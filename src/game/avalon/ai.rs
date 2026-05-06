use std::collections::HashMap;
use uuid::Uuid;
use rand::Rng;
use rand::seq::SliceRandom;

use super::engine::{AvalonGame, TeamVote, MissionVote, GamePhase};
use super::roles::{Role, Alignment, fails_needed};

#[derive(Debug, Clone)]
pub struct Personality {
    pub aggressiveness: f32,
    pub randomness: f32,
    pub deception: f32,
}

impl Personality {
    pub fn new(difficulty: &str) -> Self {
        match difficulty {
            "easy" => Personality { aggressiveness: 0.2, randomness: 0.6, deception: 0.1 },
            "hard" => Personality { aggressiveness: 0.7, randomness: 0.1, deception: 0.5 },
            _ => Personality { aggressiveness: 0.45, randomness: 0.25, deception: 0.3 },
        }
    }
}

#[derive(Debug, Clone)]
pub struct AIPlayer {
    pub id: Uuid,
    pub name: String,
    pub role: Role,
    pub difficulty: String,
    pub personality: Personality,
    pub suspicion: HashMap<Uuid, f32>,
    pub merlin_prob: HashMap<Uuid, f32>,
    pub history: Vec<GameEventRecord>,
}

#[derive(Debug, Clone)]
pub struct GameEventRecord {
    pub round: usize,
    pub event_type: String,
    pub mission_success: Option<bool>,
    pub team_members: Vec<Uuid>,
    pub votes: HashMap<Uuid, String>,
}

impl AIPlayer {
    pub fn new(id: Uuid, name: String, role: Role, difficulty: &str) -> Self {
        AIPlayer {
            id,
            name,
            role,
            difficulty: difficulty.to_string(),
            personality: Personality::new(difficulty),
            suspicion: HashMap::new(),
            merlin_prob: HashMap::new(),
            history: vec![],
        }
    }

    pub fn init_knowledge(&mut self, player_ids: &[Uuid], game: &AvalonGame) {
        for pid in player_ids {
            if *pid != self.id {
                self.suspicion.insert(*pid, 0.3);
                self.merlin_prob.insert(*pid, 0.1);
            }
        }

        match self.role {
            Role::Merlin => {
                for pid in player_ids {
                    if let Some(role) = game.roles.get(pid) {
                        if role.alignment() == Alignment::Evil && *role != Role::Mordred {
                            self.suspicion.insert(*pid, 0.95);
                        }
                    }
                }
            }
            Role::Assassin | Role::Minion | Role::Morgana | Role::Mordred => {
                for pid in player_ids {
                    if let Some(role) = game.roles.get(pid) {
                        if role.alignment() == Alignment::Evil && *role != Role::Oberon && *pid != self.id {
                            self.suspicion.insert(*pid, -1.0); // known ally
                        }
                    }
                }
            }
            Role::Percival => {
                for pid in player_ids {
                    if let Some(role) = game.roles.get(pid) {
                        if *role == Role::Merlin || *role == Role::Morgana {
                            self.suspicion.insert(*pid, 0.1); // trust candidates
                        }
                    }
                }
            }
            _ => {}
        }
    }

    pub fn vote_on_team(&self, team: &[Uuid], game: &AvalonGame, rng: &mut impl Rng) -> TeamVote {
        let avg_suspicion: f32 = team.iter()
            .map(|pid| self.suspicion.get(pid).copied().unwrap_or(0.3))
            .sum::<f32>() / team.len().max(1) as f32;

        let mut should_approve = avg_suspicion < 0.45;

        // Role-specific adjustments
        match self.role {
            Role::Merlin => {
                let has_bad = team.iter().any(|pid| {
                    game.roles.get(pid).map_or(false, |r| r.alignment() == Alignment::Evil && *r != Role::Mordred)
                });
                if has_bad && rng.gen_bool((1.0 - self.personality.deception) as f64) {
                    should_approve = false;
                }
            }
            Role::Assassin | Role::Morgana => {
                let has_bad = team.iter().any(|pid| {
                    self.suspicion.get(pid).copied().unwrap_or(0.0) < 0.0
                });
                if has_bad { should_approve = true; }
                if self.personality.deception > 0.4 && rng.gen_bool(0.5) {
                    should_approve = !should_approve;
                }
            }
            Role::Oberon => {
                should_approve = rng.gen_bool(0.45);
            }
            _ => {}
        }

        if rng.gen_bool(self.personality.randomness as f64) {
            should_approve = !should_approve;
        }

        if should_approve { TeamVote::Approve } else { TeamVote::Reject }
    }

    pub fn select_team(&self, player_ids: &[Uuid], team_size: usize, game: &AvalonGame, rng: &mut impl Rng) -> Vec<Uuid> {
        let mut scored: Vec<(Uuid, f32)> = player_ids.iter().map(|pid| {
            let base = self.suspicion.get(pid).copied().unwrap_or(0.3);
            let noise: f32 = rng.gen_range(-self.personality.randomness..self.personality.randomness);
            (*pid, base + noise)
        }).collect();

        if self.role.alignment() == Alignment::Evil && self.role != Role::Oberon {
            // Include at most 1 evil ally
            let allies: Vec<Uuid> = player_ids.iter()
                .filter(|pid| self.suspicion.get(pid).copied().unwrap_or(0.0) < 0.0)
                .copied()
                .collect();
            if !allies.is_empty() && rng.gen_bool(self.personality.aggressiveness as f64) {
                let ally_idx = rng.gen_range(0..allies.len());
                let ally = allies[ally_idx];
                scored.retain(|(pid, _)| *pid != ally);
                scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                let mut team: Vec<Uuid> = scored.iter().take(team_size - 1).map(|(id, _)| *id).collect();
                team.push(ally);
                return team;
            }
        }

        scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        scored.iter().take(team_size).map(|(id, _)| *id).collect()
    }

    pub fn choose_mission_action(&self, rng: &mut impl Rng) -> MissionVote {
        if self.role.alignment() == Alignment::Good {
            return MissionVote::Success;
        }

        let fail_chance = match self.role {
            Role::Assassin => 0.3 + self.personality.aggressiveness * 0.5,
            Role::Morgana => 0.25 + self.personality.aggressiveness * 0.3,
            Role::Oberon => 0.4,
            _ => 0.3,
        };

        if rng.gen_bool(fail_chance as f64) { MissionVote::Fail } else { MissionVote::Success }
    }

    pub fn choose_assassination_target(&self, player_ids: &[Uuid], rng: &mut impl Rng) -> Uuid {
        let mut candidates: Vec<_> = player_ids.iter()
            .filter(|pid| **pid != self.id)
            .map(|pid| {
                let prob = self.merlin_prob.get(pid).copied().unwrap_or(0.1);
                let noise: f32 = rng.gen_range(-self.personality.randomness..self.personality.randomness);
                (*pid, (prob + noise).max(0.0))
            })
            .collect();

        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        candidates.first().map(|(id, _)| *id).unwrap_or(player_ids[0])
    }

    pub fn update_suspicion(&mut self, event: &GameEventRecord, rng: &mut impl Rng) {
        if let Some(success) = event.mission_success {
            let delta = if success { -0.08 } else { 0.3 };
            for pid in &event.team_members {
                if let Some(s) = self.suspicion.get_mut(pid) {
                    if *s >= 0.0 {
                        *s += delta + rng.gen_range(-0.05..0.05);
                        *s = s.clamp(0.0, 1.0);
                    }
                }
            }
        }

        for (pid, vote) in &event.votes {
            if let Some(s) = self.suspicion.get_mut(pid) {
                if *s >= 0.0 {
                    if vote == "reject" && event.mission_success == Some(true) {
                        *s += 0.05;
                    } else if vote == "approve" && event.mission_success == Some(false) {
                        *s += 0.1;
                    }
                    *s = s.clamp(0.0, 1.0);
                }
            }
        }
    }

    pub fn update_merlin_prob(&mut self, event: &GameEventRecord) {
        if let Some(success) = event.mission_success {
            for pid in &event.team_members {
                if let Some(p) = self.merlin_prob.get_mut(pid) {
                    if success { *p += 0.03; } else { *p -= 0.02; }
                    *p = p.clamp(0.0, 1.0);
                }
            }
        }
    }

    pub fn ai_speak(&self, game: &AvalonGame, rng: &mut impl Rng) -> String {
        let names = &game.players;
        let speeches = match self.role {
            Role::Merlin => {
                let evils: Vec<&str> = names.iter()
                    .filter(|p| game.roles.get(&p.user_id).map_or(false, |r| r.alignment() == Alignment::Evil && *r != Role::Mordred))
                    .map(|p| p.username.as_str()).collect();
                if evils.is_empty() {
                    vec!["我认为我们需要认真思考每次任务的人选。".into()]
                } else {
                    vec![format!("作为一名有洞察力的玩家，我认为我们应该谨慎选择任务成员。"), format!("我的直觉告诉我，有些不太对劲的地方需要大家注意。")]
                }
            }
            Role::Percival => vec![
                "我会尽力帮助好人阵营取得胜利。".into(),
                "让我们仔细分析每个人的发言和投票。".into(),
                "我觉得我们需要团结一致。".into(),
            ],
            Role::LoyalServant => vec![
                "我相信队长的判断，支持这个队伍。".into(),
                "我是忠臣，会全力协助完成任务。".into(),
                "请大家注意可疑的行为。".into(),
                "我认为我们应该相信大多数人的意见。".into(),
            ],
            Role::Assassin => {
                let merlin_hint: Vec<&str> = names.iter()
                    .filter(|p| self.merlin_prob.get(&p.user_id).copied().unwrap_or(0.0) > 0.3)
                    .map(|p| p.username.as_str()).collect();
                if merlin_hint.is_empty() {
                    vec!["我觉得这个队伍配置还不错。".into(), "我支持这种方式。".into()]
                } else {
                    vec![format!("从分析来看，某些人的发言很有信息量，大家好好观察。"), "作为经验丰富的玩家，我同意这个提议。".into()]
                }
            }
            Role::Morgana => vec![
                "我相信队长的决定是正确的。".into(),
                "作为有经验的玩家，我支持好人阵营。".into(),
                "大家看，一切都很正常，不用担心。".into(),
                "我认为这个团队非常可靠。".into(),
            ],
            Role::Mordred => vec![
                "我基本同意目前的方案。".into(),
                "继续按照这个节奏推进就好。".into(),
            ],
            Role::Oberon => vec![
                "我有点担心这支队伍的人员构成。".into(),
                "请大家慎重考虑。".into(),
                "我不确定某些人是否真的可靠。".into(),
            ],
            Role::Minion => vec![
                "我赞成队长的提议。".into(),
                "看起来一切都很妥当。".into(),
            ],
        };
        let idx = rng.gen_range(0..speeches.len());
        speeches[idx].clone()
    }
}

pub struct AIController {
    pub players: Vec<AIPlayer>,
}

impl AIController {
    pub fn new() -> Self {
        AIController { players: vec![] }
    }

    pub fn init_players(&mut self, player_ids: &[Uuid], names: &[String], roles: &HashMap<Uuid, Role>, game: &AvalonGame, difficulties: &[String]) {
        self.players.clear();
        for (i, pid) in player_ids.iter().enumerate() {
            let role = roles.get(pid).copied().unwrap_or(Role::LoyalServant);
            let diff = difficulties.get(i).map(|s| s.as_str()).unwrap_or("normal");
            let mut ai = AIPlayer::new(*pid, names[i].clone(), role, diff);
            let all_pids: Vec<Uuid> = player_ids.to_vec();
            ai.init_knowledge(&all_pids, game);
            self.players.push(ai);
        }
    }

    pub fn get_player(&self, id: Uuid) -> Option<&AIPlayer> {
        self.players.iter().find(|p| p.id == id)
    }

    pub fn get_player_mut(&mut self, id: Uuid) -> Option<&mut AIPlayer> {
        self.players.iter_mut().find(|p| p.id == id)
    }

    pub fn record_event(&mut self, event: GameEventRecord) {
        let mut rng = rand::thread_rng();
        for p in &mut self.players {
            p.update_suspicion(&event, &mut rng);
            p.update_merlin_prob(&event);
            p.history.push(event.clone());
        }
    }
}
