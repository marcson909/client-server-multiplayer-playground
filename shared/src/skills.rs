use bevy::{prelude::*, utils::HashMap};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SkillType {
    Woodcutting,
    Fishing,
    Mining,
    Combat,
}

#[derive(Serialize, Deserialize, Clone, Debug, Component)]
pub struct Skills {
    pub skills: HashMap<SkillType, SkillData>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SkillData {
    pub level: u32,
    pub experience: u32,
}

impl Skills {
    pub fn new() -> Self {
        let mut skills = HashMap::new();
        skills.insert(
            SkillType::Woodcutting,
            SkillData {
                level: 1,
                experience: 0,
            },
        );
        skills.insert(
            SkillType::Fishing,
            SkillData {
                level: 1,
                experience: 0,
            },
        );
        skills.insert(
            SkillType::Mining,
            SkillData {
                level: 1,
                experience: 0,
            },
        );
        skills.insert(
            SkillType::Combat,
            SkillData {
                level: 1,
                experience: 0,
            },
        );
        Self { skills }
    }

    pub fn add_experience(&mut self, skill: SkillType, xp: u32) -> bool {
        if let Some(skill_data) = self.skills.get_mut(&skill) {
            skill_data.experience += xp;
            let new_level = Self::calculate_level(skill_data.experience);
            if new_level > skill_data.level {
                skill_data.level = new_level;
                return true;
            }
        }
        false
    }

    pub fn get_level(&self, skill: SkillType) -> u32 {
        self.skills.get(&skill).map(|s| s.level).unwrap_or(1)
    }

    pub fn get_experience(&self, skill: SkillType) -> u32 {
        self.skills.get(&skill).map(|s| s.experience).unwrap_or(0)
    }

    fn calculate_level(xp: u32) -> u32 {
        let mut level: u32 = 1 as u32;
        let mut xp_needed = 0;

        while xp_needed <= xp {
            level += 1;
            xp_needed += (level as f32 + 300.0 * 2_f32.powf(level as f32 / 7.0)).floor() as u32 / 4;
            if level >= 99 {
                break;
            }
        }

        level.saturating_sub(1).max(1)
    }
}
