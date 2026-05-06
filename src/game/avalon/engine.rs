use serde::{Deserialize, Serialize};
use uuid::Uuid;
use rand::seq::SliceRandom;
use rand::thread_rng;
use std::collections::{HashMap, HashSet};

use super::roles::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GamePhase { RoleReveal, Proposal, Discussion, Vote, Mission, Result, Assassination, End }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamVote { Approve, Reject }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MissionVote { Success, Fail }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub user_id: Uuid, pub username: String, pub avatar: String,
    pub role: Option<Role>, pub alignment: Option<Alignment>, pub is_leader: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssassinationViewInfo {
    pub alignment: Alignment,
    pub role: Option<Role>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundRecord {
    pub round: usize, pub leader_id: Uuid, pub leader_name: String,
    pub team: Vec<Uuid>, pub team_votes: HashMap<Uuid, String>,
    pub team_approved: bool,
    pub mission_success: bool, pub mission_vote_count: usize,
    pub success_count: usize, pub fail_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeakingState {
    pub queue: Vec<Uuid>,
    pub current_idx: usize,
    pub started_at: i64,
    pub timeout_secs: u64,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageTimer {
    pub stage: GamePhase,
    pub started_at: i64,
    pub timeout_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvalonGame {
    pub players: Vec<PlayerInfo>, pub roles: HashMap<Uuid, Role>, pub phase: GamePhase,
    pub round: usize, pub mission_results: Vec<Option<bool>>, pub consecutive_veto: usize,
    pub current_leader_idx: usize, pub mission_team: Vec<Uuid>, pub mission_sizes: Vec<usize>,
    pub votes_needed: usize,
    pub collected_team_votes: HashMap<Uuid, TeamVote>,
    pub collected_mission_votes: HashMap<Uuid, MissionVote>,
    pub winner: Option<Alignment>, pub assassin_target: Option<Uuid>,
    pub ai_player_ids: HashSet<Uuid>, pub round_history: Vec<RoundRecord>,
    pub last_team_votes: HashMap<Uuid, String>,
    pub settlement_confirmed: HashSet<Uuid>,
    pub disconnected_players: HashMap<Uuid, i64>,
    pub ai_takeover_players: HashSet<Uuid>,
    pub speaking: SpeakingState,
    pub stage_timer: StageTimer,
    pub proposal_ready: bool,
}

impl AvalonGame {
    pub fn new(player_infos: Vec<(Uuid, String, String)>) -> Self {
        Self::new_with_roles(player_infos, None)
    }

    pub fn new_with_roles(player_infos: Vec<(Uuid, String, String)>, custom_roles: Option<Vec<Role>>) -> Self {
        let player_count = player_infos.len();
        let roles_list = custom_roles.unwrap_or_else(|| get_recommended_roles(player_count));
        let mission_sizes = get_mission_sizes(player_count);
        let mut rng = thread_rng();
        let mut shuffled = roles_list.clone();
        shuffled.shuffle(&mut rng);
        let mut roles = HashMap::new();
        let players = player_infos.iter().enumerate().map(|(i, (uid, name, avatar))| {
            let role = shuffled.get(i).copied().unwrap_or(Role::LoyalServant);
            roles.insert(*uid, role);
            PlayerInfo { user_id: *uid, username: name.clone(), avatar: avatar.clone(), role: Some(role), alignment: Some(role.alignment()), is_leader: i == 0 }
        }).collect();
        let now = now_secs();
        let mut result = AvalonGame {
            players, roles, phase: GamePhase::RoleReveal, round: 1, mission_results: vec![None; 5],
            consecutive_veto: 0, current_leader_idx: 0, mission_team: vec![], mission_sizes,
            votes_needed: player_count,
            collected_team_votes: HashMap::new(),
            collected_mission_votes: HashMap::new(), winner: None, assassin_target: None,
            ai_player_ids: HashSet::new(), round_history: vec![],
            last_team_votes: HashMap::new(),
            settlement_confirmed: HashSet::new(),
            disconnected_players: HashMap::new(),
            ai_takeover_players: HashSet::new(),
            speaking: SpeakingState { queue: vec![], current_idx: 0, started_at: 0, timeout_secs: 0, active: false },
            stage_timer: StageTimer { stage: GamePhase::RoleReveal, started_at: now, timeout_secs: 300 },
            proposal_ready: false,
        };
        result.advance_phase();
        result
    }

    pub fn now_secs() -> i64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
    }

    pub fn current_player_id(&self) -> Option<Uuid> {
        match self.phase {
            GamePhase::Proposal => self.players.get(self.current_leader_idx).map(|p| p.user_id),
            GamePhase::Discussion => self.speaking.queue.get(self.speaking.current_idx).copied(),
            GamePhase::Vote => self.players.iter().find(|p| !self.collected_team_votes.contains_key(&p.user_id)).map(|p| p.user_id),
            GamePhase::Mission => self.mission_team.iter().find(|uid| !self.collected_mission_votes.contains_key(uid)).copied(),
            GamePhase::Assassination => self.roles.iter().find(|(_, r)| **r == Role::Assassin).map(|(uid, _)| *uid),
            _ => None,
        }
    }

    fn init_proposal_speaking(&mut self) {
        let leader = self.players.get(self.current_leader_idx);
        let leader_id = leader.map(|p| p.user_id);
        if let Some(lid) = leader_id {
            if self.is_human(lid) {
                self.speaking = SpeakingState {
                    queue: vec![lid],
                    current_idx: 0,
                    started_at: now_secs(),
                    timeout_secs: 30,
                    active: true,
                };
            } else {
                self.proposal_ready = true;
                self.speaking.active = false;
            }
        }
    }

    fn init_discussion_speaking(&mut self) {
        let mut queue: Vec<Uuid> = Vec::new();
        // Skip the leader (already spoke in Proposal), start from leader+1 clockwise
        let start = (self.current_leader_idx + 1) % self.players.len();
        for offset in 1..self.players.len() {
            let idx = (start + offset - 1) % self.players.len();
            let uid = self.players[idx].user_id;
            if self.is_human(uid) {
                queue.push(uid);
            }
        }
        if queue.is_empty() {
            self.speaking.active = false;
            self.advance_phase();
            return;
        }
        self.speaking = SpeakingState {
            queue,
            current_idx: 0,
            started_at: now_secs(),
            timeout_secs: 90,
            active: true,
        };
    }

    pub fn init_speaking_for_phase(&mut self) {
        let now = now_secs();
        let timeout = match self.phase {
            GamePhase::Result => 8,
            GamePhase::Assassination => 300,
            _ => 300,
        };
        self.stage_timer = StageTimer { stage: self.phase, started_at: now, timeout_secs: timeout };
        match self.phase {
            GamePhase::Proposal => self.init_proposal_speaking(),
            GamePhase::Discussion => self.init_discussion_speaking(),
            GamePhase::Result => {
                self.speaking = SpeakingState { queue: vec![], current_idx: 0, started_at: now, timeout_secs: 10, active: false };
            }
            _ => { self.speaking.active = false; }
        }
    }

    pub fn is_human(&self, uid: Uuid) -> bool {
        !self.ai_player_ids.contains(&uid) && !self.ai_takeover_players.contains(&uid)
    }

    pub fn end_speaking(&mut self, player_id: Uuid) -> Result<bool, String> {
        if !self.speaking.active { return Err("Not in speaking phase".into()); }
        let expected = self.speaking.queue.get(self.speaking.current_idx).copied();
        if expected != Some(player_id) { return Err("Not your turn".into()); }
        self.advance_speaker();
        Ok(self.speaking.active)
    }

    pub fn advance_speaker(&mut self) {
        if !self.speaking.active { return; }
        self.speaking.current_idx += 1;
        if self.speaking.current_idx >= self.speaking.queue.len() {
            self.speaking.active = false;
            self.speaking.queue.clear();
            self.speaking.current_idx = 0;
            match self.phase {
                GamePhase::Proposal => { self.proposal_ready = true; }
                GamePhase::Discussion => { self.proposal_ready = false; self.advance_leader(); }
                _ => {}
            }
        } else {
            self.speaking.started_at = now_secs();
        }
    }

    pub fn check_timeout(&mut self) -> bool {
        let now = now_secs();
        if self.speaking.active {
            if now - self.speaking.started_at > self.speaking.timeout_secs as i64 {
                self.advance_speaker();
                return true;
            }
        }
        if self.phase != GamePhase::End && now - self.stage_timer.started_at > self.stage_timer.timeout_secs as i64 {
            self.advance_phase();
            return true;
        }
        false
    }

    pub fn advance_phase(&mut self) {
        match self.phase {
            GamePhase::RoleReveal => { self.phase = GamePhase::Proposal; self.collected_team_votes.clear(); self.collected_mission_votes.clear(); self.mission_team.clear(); self.consecutive_veto = 0; self.init_speaking_for_phase(); }
            GamePhase::Proposal => {
                if !self.proposal_ready {
                    self.init_speaking_for_phase();
                    return;
                }
                self.speaking.active = false;
                self.phase = GamePhase::Discussion;
                self.init_speaking_for_phase();
            }
            GamePhase::Discussion => {
                if self.speaking.active { return; }
                self.phase = GamePhase::Vote;
                self.collected_team_votes.clear();
                self.init_speaking_for_phase();
            }
            GamePhase::Vote => {
                let approves = self.collected_team_votes.values().filter(|v| **v == TeamVote::Approve).count();
                let needed = self.active_votes_needed();
                self.last_team_votes = self.collected_team_votes.iter().map(|(k, v)| (*k, match v { TeamVote::Approve => "approve".into(), TeamVote::Reject => "reject".into() })).collect();
                if approves as f64 > (needed as f64 / 2.0) {
                    self.consecutive_veto = 0; self.phase = GamePhase::Mission; self.collected_mission_votes.clear();
                    self.speaking.active = false;
                } else {
                    let leader = self.players.get(self.current_leader_idx);
                    self.round_history.push(RoundRecord {
                        round: self.round, leader_id: leader.map(|p| p.user_id).unwrap_or_default(),
                        leader_name: leader.map(|p| p.username.clone()).unwrap_or_default(),
                        team: self.mission_team.clone(), team_votes: self.last_team_votes.clone(),
                        team_approved: false, mission_success: false, mission_vote_count: 0, success_count: 0, fail_count: 0,
                    });
                    self.consecutive_veto += 1;
                    if self.consecutive_veto >= 5 { self.consecutive_veto = 0; self.phase = GamePhase::Mission; self.collected_mission_votes.clear(); self.speaking.active = false; }
                    else { self.advance_leader(); self.mission_team.clear(); self.phase = GamePhase::Proposal; self.proposal_ready = false; self.init_speaking_for_phase(); }
                }
                self.collected_team_votes.clear();
            }
            GamePhase::Mission => {
                let successes = self.collected_mission_votes.values().filter(|v| **v == MissionVote::Success).count();
                let fails = self.collected_mission_votes.len() - successes;
                let fails_required = fails_needed(self.round - 1, self.players.len());
                let mission_success = fails < fails_required;
                let leader = self.players.get(self.current_leader_idx);
                self.round_history.push(RoundRecord {
                    round: self.round, leader_id: leader.map(|p| p.user_id).unwrap_or_default(),
                    leader_name: leader.map(|p| p.username.clone()).unwrap_or_default(),
                    team: self.mission_team.clone(), team_votes: self.last_team_votes.clone(),
                    team_approved: true, mission_success, mission_vote_count: self.collected_mission_votes.len(),
                    success_count: successes, fail_count: fails,
                });
                self.mission_results[self.round - 1] = Some(mission_success);
                let good_wins = self.mission_results.iter().filter(|r| **r == Some(true)).count();
                let evil_wins = self.mission_results.iter().filter(|r| **r == Some(false)).count();
                self.collected_mission_votes.clear(); self.collected_team_votes.clear();
                if evil_wins >= 3 { self.winner = Some(Alignment::Evil); self.phase = GamePhase::End; }
                else if good_wins >= 3 {
                    if self.has_assassin() { self.phase = GamePhase::Assassination; self.init_speaking_for_phase(); }
                    else { self.winner = Some(Alignment::Good); self.phase = GamePhase::End; }
                } else { 
                    // Go through Result phase to show outcome, then auto-advance
                    self.phase = GamePhase::Result; 
                    self.init_speaking_for_phase(); 
                    self.round += 1;
                    // Immediately advance to next round's Proposal
                    self.advance_phase();
                }
            }
            GamePhase::Result => {
                self.advance_leader(); self.consecutive_veto = 0;
                self.proposal_ready = false;
                self.phase = GamePhase::Proposal; self.mission_team.clear();
                self.init_speaking_for_phase();
            }
            _ => {}
        }
    }

    pub fn advance_team_selection(&mut self) {
        if self.phase == GamePhase::Proposal && self.proposal_ready {
            self.advance_phase();
        }
    }

    fn advance_leader(&mut self) {
        self.current_leader_idx = (self.current_leader_idx + 1) % self.players.len();
        for p in &mut self.players { p.is_leader = false; }
        if let Some(p) = self.players.get_mut(self.current_leader_idx) { p.is_leader = true; }
    }

    fn has_assassin(&self) -> bool { self.roles.values().any(|r| *r == Role::Assassin) }

    fn active_votes_needed(&self) -> usize {
        let active = self.players.iter().filter(|p| self.is_human(p.user_id)).count();
        if active == 0 { 1 } else { (active / 2) + 1 }
    }

    pub fn select_team(&mut self, user_id: Uuid, team: Vec<Uuid>) -> Result<(), String> {
        if self.phase != GamePhase::Proposal { return Err("Not in proposal phase".into()); }
        if !self.proposal_ready { return Err("Must finish speaking first".into()); }
        let leader_id = self.players.get(self.current_leader_idx).map(|p| p.user_id).unwrap_or_default();
        if user_id != leader_id { return Err("Only the leader can select team".into()); }
        let round_size = self.mission_sizes.get(self.round - 1).copied().unwrap_or(2);
        if team.len() != round_size { return Err(format!("Need exactly {} members", round_size)); }
        for tid in &team { if !self.players.iter().any(|p| p.user_id == *tid) { return Err(format!("Player {} not in game", tid)); } }
        self.mission_team = team;
        Ok(())
    }

    pub fn submit_team_vote(&mut self, user_id: Uuid, vote: TeamVote) -> Result<(bool, bool), String> {
        if self.phase != GamePhase::Vote { return Err("Not in vote phase".into()); }
        if !self.players.iter().any(|p| p.user_id == user_id) { return Err("Player not in game".into()); }
        self.collected_team_votes.insert(user_id, vote);
        let needed = self.active_votes_needed();
        let all_voted = self.collected_team_votes.len() >= needed;
        if all_voted { self.advance_phase(); }
        Ok((all_voted, all_voted))
    }

    pub fn submit_mission_vote(&mut self, user_id: Uuid, vote: MissionVote) -> Result<(bool, bool), String> {
        if self.phase != GamePhase::Mission { return Err("Not in mission phase".into()); }
        if !self.mission_team.contains(&user_id) { return Err("You are not on the mission team".into()); }
        self.collected_mission_votes.insert(user_id, vote);
        let all_voted = self.collected_mission_votes.len() >= self.mission_team.len();
        if all_voted { self.advance_phase(); }
        Ok((all_voted, all_voted))
    }

    pub fn assassinate(&mut self, user_id: Uuid, target_id: Uuid) -> Result<Option<Alignment>, String> {
        if self.phase != GamePhase::Assassination { return Err("Not in assassination phase".into()); }
        let assassin_id = self.roles.iter().find(|(_, r)| **r == Role::Assassin).map(|(uid, _)| *uid);
        if assassin_id.map_or(true, |a| a != user_id) { return Err("Only the assassin can assassinate".into()); }
        let target_role = self.roles.get(&target_id).copied();
        self.assassin_target = Some(target_id);
        self.winner = if target_role == Some(Role::Merlin) { Some(Alignment::Evil) } else { Some(Alignment::Good) };
        self.phase = GamePhase::End;
        self.speaking.active = false;
        Ok(self.winner)
    }

    pub fn confirm_settlement(&mut self, player_id: Uuid) -> bool {
        self.settlement_confirmed.insert(player_id);
        let human_count = self.players.iter().filter(|p| self.is_human(p.user_id)).count();
        let all_confirmed = self.settlement_confirmed.len() >= human_count;
        if all_confirmed { self.phase = GamePhase::End; }
        all_confirmed
    }

    pub fn mark_disconnected(&mut self, player_id: Uuid) {
        let now = now_secs();
        self.disconnected_players.insert(player_id, now);
        // If this player was speaking, advance
        if let Some(current) = self.speaking.queue.get(self.speaking.current_idx).copied() {
            if current == player_id { self.advance_speaker(); }
        }
    }

    pub fn try_ai_takeover(&mut self, player_id: Uuid) -> bool {
        if let Some(disconnected_at) = self.disconnected_players.get(&player_id) {
            if now_secs() - disconnected_at >= 60 {
                self.ai_takeover_players.insert(player_id);
                if let Some(current) = self.speaking.queue.get(self.speaking.current_idx).copied() {
                    if current == player_id { self.advance_speaker(); }
                }
                return true;
            }
        }
        false
    }

    pub fn get_player_view(&self, player_id: Uuid) -> PlayerGameView {
        let my_role = self.roles.get(&player_id).copied();
        let mut visible_evil: HashMap<Uuid, bool> = HashMap::new();
        let mut visible_roles_map: HashMap<Uuid, Option<Role>> = HashMap::new();
        if let Some(role) = my_role {
            for p in &self.players {
                let other_role = self.roles.get(&p.user_id).copied();
                match role {
                    Role::Merlin => { if other_role.map_or(false, |r| r.alignment() == Alignment::Evil && r != Role::Mordred) { visible_evil.insert(p.user_id, true); } }
                    Role::Percival => { if other_role == Some(Role::Merlin) || other_role == Some(Role::Morgana) { visible_roles_map.insert(p.user_id, other_role); } }
                    Role::Assassin | Role::Minion | Role::Morgana | Role::Mordred => { if other_role.map_or(false, |r| r.alignment() == Alignment::Evil && r != Role::Oberon) { visible_evil.insert(p.user_id, true); } }
                    _ => {}
                }
            }
        }
        let assassination_visibility: HashMap<Uuid, AssassinationViewInfo> = if self.phase == GamePhase::Assassination || self.phase == GamePhase::End {
            self.roles.iter().map(|(uid, role)| {
                let align = role.alignment();
                let show_role = if align == Alignment::Evil { Some(*role) } else { None };
                (*uid, AssassinationViewInfo { alignment: align, role: show_role })
            }).collect()
        } else { HashMap::new() };
        PlayerGameView {
            your_role: my_role, your_alignment: my_role.map(|r| r.alignment()),
            phase: self.phase, round: self.round,
            players: self.players.iter().map(|p| PlayerViewInfo {
                user_id: p.user_id, username: p.username.clone(), avatar: p.avatar.clone(),
                is_leader: p.is_leader,
                known_evil: *visible_evil.get(&p.user_id).unwrap_or(&false),
                known_role: visible_roles_map.get(&p.user_id).and_then(|r| *r),
                is_connected: !self.disconnected_players.contains_key(&p.user_id),
                is_ai_controlled: self.ai_takeover_players.contains(&p.user_id),
            }).collect(),
            mission_team: self.mission_team.clone(),
            mission_results: self.mission_results.clone(),
            mission_sizes: self.mission_sizes.clone(),
            consecutive_veto: self.consecutive_veto,
            winner: self.winner,
            is_your_turn: self.is_player_turn(player_id),
            all_roles: if self.phase == GamePhase::End { self.roles.iter().map(|(k, v)| (*k, *v)).collect() } else { HashMap::new() },
            round_history: self.round_history.clone(),
            settlement_confirmed: self.settlement_confirmed.clone(),
            assassin_target: self.assassin_target,
            speaking_phase: self.speaking.active,
            speaking_queue: self.speaking.queue.clone(),
            current_speaker: self.speaking.queue.get(self.speaking.current_idx).copied(),
            speaking_remaining: if self.speaking.active { self.speaking.timeout_secs as i64 - (now_secs() - self.speaking.started_at).max(0) } else { 0 },
            proposal_ready: self.proposal_ready,
            assassination_visibility,
        }
    }

    fn is_player_turn(&self, player_id: Uuid) -> bool {
        match self.phase {
            GamePhase::Proposal => self.speaking.active && self.speaking.queue.get(0).copied() == Some(player_id),
            GamePhase::Discussion => self.speaking.active && self.speaking.queue.get(self.speaking.current_idx).copied() == Some(player_id),
            GamePhase::Vote => !self.collected_team_votes.contains_key(&player_id),
            GamePhase::Mission => self.mission_team.contains(&player_id) && !self.collected_mission_votes.contains_key(&player_id),
            GamePhase::Assassination => self.roles.get(&player_id).copied() == Some(Role::Assassin),
            _ => false,
        }
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerViewInfo { pub user_id: Uuid, pub username: String, pub avatar: String, pub is_leader: bool, pub known_evil: bool, pub known_role: Option<Role>, pub is_connected: bool, pub is_ai_controlled: bool }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerGameView {
    pub your_role: Option<Role>, pub your_alignment: Option<Alignment>,
    pub phase: GamePhase, pub round: usize, pub players: Vec<PlayerViewInfo>,
    pub mission_team: Vec<Uuid>, pub mission_results: Vec<Option<bool>>,
    pub mission_sizes: Vec<usize>, pub consecutive_veto: usize,
    pub winner: Option<Alignment>, pub is_your_turn: bool, pub all_roles: HashMap<Uuid, Role>,
    pub round_history: Vec<RoundRecord>,
    pub settlement_confirmed: HashSet<Uuid>,
    pub assassin_target: Option<Uuid>,
    pub speaking_phase: bool,
    pub speaking_queue: Vec<Uuid>,
    pub current_speaker: Option<Uuid>,
    pub speaking_remaining: i64,
    pub proposal_ready: bool,
    pub assassination_visibility: HashMap<Uuid, AssassinationViewInfo>,
}
