// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Based on TrinityCore protocol research (https://github.com/TrinityCore/TrinityCore)
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! Gossip packets: TalkToGossip (Hello), GossipMessage, GossipComplete.
//!
//! These handle NPC interaction when a player right-clicks a creature.

use wow_constants::{ClientOpcodes, ServerOpcodes};
use wow_core::ObjectGuid;

use crate::world_packet::PacketError;
use crate::{ClientPacket, ServerPacket, WorldPacket};

// ── CMSG_GOSSIP_HELLO / TalkToGossip (0x3492) ──────────────────────

/// Client request to talk to an NPC.
///
/// Also used for: CMSG_BANKER_ACTIVATE, CMSG_BINDER_ACTIVATE,
/// CMSG_LIST_INVENTORY, CMSG_TRAINER_LIST, CMSG_BATTLEMASTER_HELLO.
pub struct Hello {
    pub unit: ObjectGuid,
}

impl ClientPacket for Hello {
    const OPCODE: ClientOpcodes = ClientOpcodes::TalkToGossip;

    fn read(packet: &mut WorldPacket) -> Result<Self, PacketError> {
        let unit = packet.read_packed_guid()?;
        Ok(Self { unit })
    }
}

// ── CMSG_GOSSIP_SELECT_OPTION (0x3494) ──────────────────────────────

/// Client selects a gossip menu option.
pub struct GossipSelectOption {
    pub gossip_unit: ObjectGuid,
    pub gossip_id: i32,
    pub gossip_option_id: i32,
    pub promotion_code: String,
}

impl ClientPacket for GossipSelectOption {
    const OPCODE: ClientOpcodes = ClientOpcodes::GossipSelectOption;

    fn read(packet: &mut WorldPacket) -> Result<Self, PacketError> {
        let gossip_unit = packet.read_packed_guid()?;
        let gossip_id = packet.read_int32()?;
        let gossip_option_id = packet.read_int32()?;
        let len = packet.read_bits(8)? as usize;
        let promotion_code = if len > 0 {
            let bytes = packet.read_bytes(len)?;
            String::from_utf8_lossy(&bytes).to_string()
        } else {
            String::new()
        };
        Ok(Self {
            gossip_unit,
            gossip_id,
            gossip_option_id,
            promotion_code,
        })
    }
}

// ── SMSG_GOSSIP_MESSAGE (0x2A98) ────────────────────────────────────

/// A single gossip menu option.
pub struct ClientGossipOption {
    pub gossip_option_id: i32,
    pub option_npc: u8,
    pub option_flags: i8,
    pub option_cost: i32,
    pub option_language: i32,
    pub flags: i32,
    pub order_index: i32,
    pub status: u8, // 0=Available, 1=Unavailable, 2=Locked
    pub text: String,
    pub confirm: String,
    pub spell_id: Option<i32>,
    pub override_icon_id: Option<i32>,
}

/// A quest entry in a gossip menu.
///
/// C++ source of truth:
/// - `Server/Packets/NPCPackets.h::ClientGossipText`
/// - `Server/Packets/NPCPackets.cpp::operator<<(ClientGossipText const&)`
///
/// The configured 3.4.3 C++ target writes exactly two `QuestFlags` ints and
/// does not write `LfgDungeonsID`, `Unused1102`, `QuestFlags[2]`,
/// `ResetByScheduler`, or `Meta` in this packet layout.
pub struct ClientGossipText {
    pub quest_id: i32,
    pub content_tuning_id: i32,
    pub quest_type: i32,
    pub quest_level: i32,
    pub quest_max_scaling_level: i32,
    pub quest_flags: u32,
    pub quest_flags_ex: u32,
    pub repeatable: bool,
    pub important: bool,
    pub quest_title: String,
}

/// Server gossip message with menu options and quests.
pub struct GossipMessage {
    pub gossip_guid: ObjectGuid,
    pub gossip_id: i32,
    pub friendship_faction_id: i32,
    pub text_id: Option<i32>,
    pub broadcast_text_id: Option<i32>,
    pub gossip_options: Vec<ClientGossipOption>,
    pub gossip_text: Vec<ClientGossipText>,
}

impl ServerPacket for GossipMessage {
    const OPCODE: ServerOpcodes = ServerOpcodes::GossipMessage;

    fn write(&self, pkt: &mut WorldPacket) {
        pkt.write_packed_guid(&self.gossip_guid);
        pkt.write_int32(self.gossip_id);
        pkt.write_int32(self.friendship_faction_id);
        pkt.write_int32(self.gossip_options.len() as i32);
        pkt.write_int32(self.gossip_text.len() as i32);
        pkt.write_bit(self.text_id.is_some());
        pkt.write_bit(self.broadcast_text_id.is_some());
        pkt.flush_bits();

        // Gossip options
        for opt in &self.gossip_options {
            pkt.write_int32(opt.gossip_option_id);
            pkt.write_uint8(opt.option_npc);
            pkt.write_int8(opt.option_flags);
            pkt.write_int32(opt.option_cost);
            pkt.write_int32(opt.option_language);
            pkt.write_int32(opt.flags);
            pkt.write_int32(opt.order_index);
            pkt.write_bits(opt.text.len() as u32, 12);
            pkt.write_bits(opt.confirm.len() as u32, 12);
            pkt.write_bits(opt.status as u32, 2);
            pkt.write_bit(opt.spell_id.is_some());
            pkt.write_bit(opt.override_icon_id.is_some());
            pkt.flush_bits();

            // TreasureLootList (empty)
            pkt.write_int32(0); // Items.Count = 0

            pkt.write_string(&opt.text);
            pkt.write_string(&opt.confirm);

            if let Some(spell_id) = opt.spell_id {
                pkt.write_int32(spell_id);
            }
            if let Some(icon_id) = opt.override_icon_id {
                pkt.write_int32(icon_id);
            }
        }

        // TextID (optional)
        if let Some(text_id) = self.text_id {
            pkt.write_int32(text_id);
        }

        // BroadcastTextID (optional)
        if let Some(broadcast_text_id) = self.broadcast_text_id {
            pkt.write_int32(broadcast_text_id);
        }

        // Gossip text (quests)
        for text in &self.gossip_text {
            pkt.write_int32(text.quest_id);
            pkt.write_int32(text.content_tuning_id);
            pkt.write_int32(text.quest_type);
            pkt.write_int32(text.quest_level);
            pkt.write_int32(text.quest_max_scaling_level);
            pkt.write_uint32(text.quest_flags);
            pkt.write_uint32(text.quest_flags_ex);
            pkt.write_bit(text.repeatable);
            pkt.write_bit(text.important);
            pkt.write_bits(text.quest_title.len() as u32, 9);
            pkt.flush_bits();
            pkt.write_string(&text.quest_title);
        }
    }
}

impl GossipMessage {
    /// Create an empty gossip message (no options, no quests).
    ///
    /// This is what the client sees when an NPC has no gossip options.
    /// The NPC text (TextID) controls what the NPC "says" in the window.
    pub fn empty(guid: ObjectGuid, gossip_id: i32, text_id: i32) -> Self {
        Self {
            gossip_guid: guid,
            gossip_id,
            friendship_faction_id: 0,
            text_id: Some(text_id),
            broadcast_text_id: None,
            gossip_options: Vec::new(),
            gossip_text: Vec::new(),
        }
    }
}

// ── SMSG_GOSSIP_COMPLETE (0x2A97) ───────────────────────────────────

/// Close the gossip window on the client.
pub struct GossipComplete {
    pub suppress_sound: bool,
}

impl ServerPacket for GossipComplete {
    const OPCODE: ServerOpcodes = ServerOpcodes::GossipComplete;

    fn write(&self, pkt: &mut WorldPacket) {
        pkt.write_bit(self.suppress_sound);
        pkt.flush_bits();
    }
}

// ── CMSG_QUERY_NPC_TEXT (0x3272) ────────────────────────────────────

/// Client requests NPC text for a gossip window.
pub struct QueryNpcText {
    pub text_id: u32,
    pub guid: ObjectGuid,
}

impl ClientPacket for QueryNpcText {
    const OPCODE: ClientOpcodes = ClientOpcodes::QueryNpcText;

    fn read(packet: &mut WorldPacket) -> Result<Self, PacketError> {
        let text_id = packet.read_uint32()?;
        let guid = packet.read_packed_guid()?;
        Ok(Self { text_id, guid })
    }
}

// ── SMSG_QUERY_NPC_TEXT_RESPONSE (0x2916) ───────────────────────────

/// Server response with NPC text data.
///
/// C++ `QueryNPCTextResponse::Write` (QueryPackets.cpp:316-332): `TextID`, the
/// `Allow` bit (flushed), the size of the option block (64 when `Allow`, else
/// 0; always written), then all eight probabilities followed by all eight
/// BroadcastTextIDs. HermesProxy-WOTLK `QueryNPCTextResponse.cs` agrees.
pub struct QueryNpcTextResponse {
    pub text_id: u32,
    pub allow: bool,
    pub probabilities: [f32; 8],
    pub broadcast_text_ids: [u32; 8],
}

impl ServerPacket for QueryNpcTextResponse {
    const OPCODE: ServerOpcodes = ServerOpcodes::QueryNpcTextResponse;

    fn write(&self, pkt: &mut WorldPacket) {
        pkt.write_uint32(self.text_id);
        pkt.write_bit(self.allow);
        pkt.flush_bits();

        // MAX_NPC_TEXT_OPTIONS * (sizeof(float) + sizeof(uint32))
        pkt.write_int32(if self.allow { 64 } else { 0 });
        if !self.allow {
            return;
        }
        for &probability in &self.probabilities {
            pkt.write_float(probability);
        }
        for &broadcast_text_id in &self.broadcast_text_ids {
            pkt.write_uint32(broadcast_text_id);
        }
    }
}

impl QueryNpcTextResponse {
    /// C++ `WorldSession::HandleNpcTextQueryOpcode` (QueryHandler.cpp:175-199):
    /// `Allow` is set by the first non-zero BroadcastTextID.
    pub fn from_options(
        text_id: u32,
        probabilities: [f32; 8],
        broadcast_text_ids: [u32; 8],
    ) -> Self {
        Self {
            text_id,
            allow: broadcast_text_ids.iter().any(|&id| id != 0),
            probabilities,
            broadcast_text_ids,
        }
    }

    /// Response for a TextID without an `npc_text` entry.
    pub fn not_found(text_id: u32) -> Self {
        Self {
            text_id,
            allow: false,
            probabilities: [0.0; 8],
            broadcast_text_ids: [0; 8],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gossip_message_empty_serializes() {
        let msg = GossipMessage::empty(ObjectGuid::EMPTY, 0, 1);
        let bytes = msg.to_bytes();
        // Should at least have opcode + guid + fields
        assert!(
            bytes.len() > 10,
            "GossipMessage too small: {} bytes",
            bytes.len()
        );
    }

    #[test]
    fn gossip_message_quest_text_matches_cpp_npc_client_gossip_text_layout() {
        let msg = GossipMessage {
            gossip_guid: ObjectGuid::EMPTY,
            gossip_id: 7,
            friendship_faction_id: 0,
            text_id: None,
            broadcast_text_id: None,
            gossip_options: Vec::new(),
            gossip_text: vec![ClientGossipText {
                quest_id: 10070,
                content_tuning_id: 2,
                quest_type: 3,
                quest_level: 4,
                quest_max_scaling_level: 5,
                quest_flags: 0x0102_0304,
                quest_flags_ex: 0x0506_0708,
                repeatable: true,
                important: false,
                quest_title: "Well Watcher Solanian".to_string(),
            }],
        };

        let bytes = msg.to_bytes();
        let mut pkt = WorldPacket::from_bytes(&bytes[2..]);

        assert_eq!(pkt.read_packed_guid().unwrap(), ObjectGuid::EMPTY);
        assert_eq!(pkt.read_int32().unwrap(), 7);
        assert_eq!(pkt.read_int32().unwrap(), 0);
        assert_eq!(pkt.read_int32().unwrap(), 0);
        assert_eq!(pkt.read_int32().unwrap(), 1);
        assert!(!pkt.read_bit().unwrap());
        assert!(!pkt.read_bit().unwrap());

        assert_eq!(pkt.read_int32().unwrap(), 10070);
        assert_eq!(pkt.read_int32().unwrap(), 2);
        assert_eq!(pkt.read_int32().unwrap(), 3);
        assert_eq!(pkt.read_int32().unwrap(), 4);
        assert_eq!(pkt.read_int32().unwrap(), 5);
        assert_eq!(pkt.read_uint32().unwrap(), 0x0102_0304);
        assert_eq!(pkt.read_uint32().unwrap(), 0x0506_0708);
        assert!(pkt.read_bit().unwrap());
        assert!(!pkt.read_bit().unwrap());
        let title_len = pkt.read_bits(9).unwrap() as usize;
        assert_eq!(title_len, "Well Watcher Solanian".len());
        assert_eq!(pkt.read_string(title_len).unwrap(), "Well Watcher Solanian");
        assert_eq!(
            pkt.remaining(),
            0,
            "C++ NPCPackets::ClientGossipText has exactly two QuestFlags ints and no trailing ResetByScheduler/Meta fields"
        );
    }

    #[test]
    fn gossip_complete_serializes() {
        let pkt = GossipComplete {
            suppress_sound: false,
        };
        let bytes = pkt.to_bytes();
        // opcode(2) + bit(1 byte flushed)
        assert_eq!(bytes.len(), 3);
    }

    #[test]
    fn npc_text_response_not_found() {
        let bytes = QueryNpcTextResponse::not_found(12345).to_bytes();
        // opcode(2) + TextID(4) + Allow bit flushed(1) + size 0 (4), as C++ always writes it
        assert_eq!(bytes, [0x16, 0x29, 0x39, 0x30, 0, 0, 0x00, 0, 0, 0, 0]);
    }

    #[test]
    fn npc_text_response_writes_probabilities_then_broadcast_text_ids() {
        let resp = QueryNpcTextResponse::from_options(
            1,
            [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5],
            [50429, 0, 0, 0, 0, 0, 0, 7],
        );
        assert!(resp.allow);
        let bytes = resp.to_bytes();
        assert_eq!(bytes.len(), 2 + 4 + 1 + 4 + 64);
        assert_eq!(&bytes[..11], &[0x16, 0x29, 1, 0, 0, 0, 0x80, 64, 0, 0, 0]);
        let u32_at =
            |i: usize| u32::from_le_bytes(bytes[11 + 4 * i..15 + 4 * i].try_into().unwrap());
        // probabilities[0..8] first ...
        assert_eq!(f32::from_bits(u32_at(0)), 1.0);
        assert_eq!(f32::from_bits(u32_at(1)), 0.0);
        assert_eq!(f32::from_bits(u32_at(7)), 0.5);
        // ... then BroadcastTextID[0..8]
        assert_eq!(u32_at(8), 50429);
        assert_eq!(u32_at(9), 0);
        assert_eq!(u32_at(15), 7);
    }

    #[test]
    fn npc_text_response_allow_needs_a_broadcast_text_id() {
        let resp = QueryNpcTextResponse::from_options(1, [1.0; 8], [0; 8]);
        assert!(!resp.allow);
        assert_eq!(resp.to_bytes().len(), 2 + 4 + 1 + 4);
    }
}
