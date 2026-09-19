use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttackTypeMetric {
    pub attack: String,
    pub damage_type: String,
    pub total_damage: u64,
    pub hit_count: u64,
    pub max_hit: u64,
}

pub fn attack_type_metrics(connection: &Connection) -> Result<Vec<AttackTypeMetric>, String> {
    let mut statement = connection
        .prepare(
            "SELECT attack_kind,damage_type,SUM(total_damage),SUM(hit_count),MAX(max_hit)
             FROM damage_attack_type_summaries
             GROUP BY attack_kind COLLATE NOCASE,damage_type
             ORDER BY SUM(total_damage) DESC,attack_kind COLLATE NOCASE",
        )
        .map_err(|error| error.to_string())?;
    let metrics = statement
        .query_map([], |row| {
            Ok(AttackTypeMetric {
                attack: row.get(0)?,
                damage_type: row.get(1)?,
                total_damage: row.get(2)?,
                hit_count: row.get(3)?,
                max_hit: row.get(4)?,
            })
        })
        .map_err(|error| error.to_string())?
        .map(|row| row.map_err(|error| error.to_string()))
        .collect();
    metrics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_attack_types_case_insensitively() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE damage_attack_type_summaries(
                    encounter_id INTEGER NOT NULL,
                    attack_kind TEXT NOT NULL,
                    damage_type TEXT NOT NULL,
                    total_damage INTEGER NOT NULL,
                    hit_count INTEGER NOT NULL,
                    max_hit INTEGER NOT NULL
                 );
                 INSERT INTO damage_attack_type_summaries VALUES
                    (1,'slash','melee',100,1,100),(2,'Slash','melee',50,1,50),
                    (1,'kick','melee',30,1,30),(1,'Dawncall','spell',125,1,125);",
            )
            .unwrap();
        let rows = attack_type_metrics(&connection).unwrap();
        assert_eq!(
            rows[0],
            AttackTypeMetric {
                attack: "slash".into(),
                damage_type: "melee".into(),
                total_damage: 150,
                hit_count: 2,
                max_hit: 100,
            }
        );
        assert_eq!(rows[1].attack, "Dawncall");
        assert_eq!(rows[2].attack, "kick");
    }
}
