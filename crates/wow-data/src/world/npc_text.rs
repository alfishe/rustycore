//! Immutable C++ `ObjectMgr::_npcTextStore` (`ObjectMgr::LoadNPCText`,
//! ObjectMgr.cpp:6275-6344) consumed by `CMSG_QUERY_NPC_TEXT`.

use std::collections::HashMap;

/// C++ `MAX_NPC_TEXT_OPTIONS` (NPCHandler.h).
pub const MAX_NPC_TEXT_OPTIONS_LIKE_CPP: usize = 8;

/// C++ `NpcText` (NPCHandler.h): eight `NpcTextData` options.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NpcTextLikeCpp {
    pub probabilities: [f32; MAX_NPC_TEXT_OPTIONS_LIKE_CPP],
    pub broadcast_text_ids: [u32; MAX_NPC_TEXT_OPTIONS_LIKE_CPP],
}

/// One raw `npc_text` row before `LoadNPCText` validation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NpcTextRowLikeCpp {
    pub id: u32,
    pub probabilities: [f32; MAX_NPC_TEXT_OPTIONS_LIKE_CPP],
    pub broadcast_text_ids: [u32; MAX_NPC_TEXT_OPTIONS_LIKE_CPP],
}

#[derive(Debug, Clone, Default)]
pub struct NpcTextCatalogLikeCpp {
    texts: HashMap<u32, NpcTextLikeCpp>,
}

impl NpcTextCatalogLikeCpp {
    /// Applies the `LoadNPCText` row rules in C++ order:
    /// 1. ID 0 is reserved and skipped;
    /// 2. a BroadcastTextID missing from `sBroadcastTextStore` clears that option;
    /// 3. a probability without a BroadcastTextID is cleared;
    /// 4. a row whose probability sum is <= 0 is skipped.
    pub fn from_rows_like_cpp(
        rows: impl IntoIterator<Item = NpcTextRowLikeCpp>,
        broadcast_text_exists: impl Fn(u32) -> bool,
    ) -> Self {
        let mut texts = HashMap::new();
        for row in rows {
            if row.id == 0 {
                continue;
            }
            let mut text = NpcTextLikeCpp {
                probabilities: row.probabilities,
                broadcast_text_ids: row.broadcast_text_ids,
            };
            for i in 0..MAX_NPC_TEXT_OPTIONS_LIKE_CPP {
                if text.broadcast_text_ids[i] != 0
                    && !broadcast_text_exists(text.broadcast_text_ids[i])
                {
                    text.probabilities[i] = 0.0;
                    text.broadcast_text_ids[i] = 0;
                }
            }
            for i in 0..MAX_NPC_TEXT_OPTIONS_LIKE_CPP {
                if text.probabilities[i] > 0.0 && text.broadcast_text_ids[i] == 0 {
                    text.probabilities[i] = 0.0;
                }
            }
            let probability_sum: f32 = text.probabilities.iter().sum();
            if probability_sum <= 0.0 {
                continue;
            }
            texts.insert(row.id, text);
        }
        Self { texts }
    }

    /// C++ `ObjectMgr::GetNpcText`.
    pub fn get_like_cpp(&self, id: u32) -> Option<&NpcTextLikeCpp> {
        self.texts.get(&id)
    }

    pub fn len(&self) -> usize {
        self.texts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: u32, probabilities: [f32; 8], broadcast_text_ids: [u32; 8]) -> NpcTextRowLikeCpp {
        NpcTextRowLikeCpp {
            id,
            probabilities,
            broadcast_text_ids,
        }
    }

    #[test]
    fn load_npc_text_keeps_valid_rows_and_all_eight_options() {
        let catalog = NpcTextCatalogLikeCpp::from_rows_like_cpp(
            [row(
                1,
                [0.5, 0.25, 0.0, 0.0, 0.0, 0.0, 0.0, 0.25],
                [50429, 50430, 0, 0, 0, 0, 0, 50431],
            )],
            |_| true,
        );
        let text = catalog.get_like_cpp(1).expect("valid row is stored");
        assert_eq!(
            text.broadcast_text_ids,
            [50429, 50430, 0, 0, 0, 0, 0, 50431]
        );
        assert_eq!(text.probabilities[7], 0.25);
        assert_eq!(catalog.len(), 1);
    }

    #[test]
    fn load_npc_text_applies_cpp_row_rules() {
        let catalog = NpcTextCatalogLikeCpp::from_rows_like_cpp(
            [
                // reserved ID 0
                row(
                    0,
                    [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    [7, 0, 0, 0, 0, 0, 0, 0],
                ),
                // option 1 names a BroadcastText the DB2 store does not have
                row(
                    2,
                    [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    [7, 999, 0, 0, 0, 0, 0, 0],
                ),
                // option 1 has a probability but no BroadcastTextID
                row(
                    3,
                    [0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    [7, 0, 0, 0, 0, 0, 0, 0],
                ),
                // nothing selectable after validation: skipped
                row(
                    4,
                    [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
                    [999, 0, 0, 0, 0, 0, 0, 0],
                ),
                // BroadcastTextIDs but probability sum 0: skipped
                row(5, [0.0; 8], [7, 0, 0, 0, 0, 0, 0, 0]),
            ],
            |id| id != 999,
        );
        assert!(catalog.get_like_cpp(0).is_none());
        let two = catalog.get_like_cpp(2).expect("row 2 keeps option 0");
        assert_eq!((two.probabilities[1], two.broadcast_text_ids[1]), (0.0, 0));
        assert_eq!((two.probabilities[0], two.broadcast_text_ids[0]), (0.5, 7));
        let three = catalog.get_like_cpp(3).expect("row 3 keeps option 0");
        assert_eq!(three.probabilities[1], 0.0);
        assert!(catalog.get_like_cpp(4).is_none());
        assert!(catalog.get_like_cpp(5).is_none());
        assert_eq!(catalog.len(), 2);
    }
}
