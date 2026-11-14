//! `SMSG_QUEST_GIVER_OFFER_REWARD_MESSAGE` / `SMSG_QUEST_GIVER_REQUEST_ITEMS`
//! content: rewards and texts.
//!
//! C++ anchors:
//! - `PlayerMenu::SendQuestGiverOfferReward` (GossipDef.cpp:538) sends
//!   `RewardText = quest->GetOfferRewardText()` and fills the rewards with
//!   `Quest::BuildQuestRewards(offer.Rewards, player)` (QuestDef.cpp:446):
//!   `Money = player->GetQuestMoneyReward(this)`, `XP = player->GetQuestXPReward(this)`.
//! - `PlayerMenu::SendQuestGiverRequestItems` (GossipDef.cpp:619) sends
//!   `CompletionText = quest->GetRequestItemsText()`.
//! - `Quest::LoadQuestOfferReward` / `LoadQuestRequestItems` (QuestDef.cpp:202/186)
//!   load those texts from `quest_offer_reward` / `quest_request_items`.

use super::*;
use wow_data::progression_rewards::{QuestMoneyRewardEntry, QuestMoneyRewardStore};

/// QuestXP fallback table used without a QuestXP.db2 store: difficulty 3 = 200.
const OFFER_TEST_XP_DIFFICULTY: u32 = 3;
const OFFER_TEST_MONEY_DIFFICULTY: u32 = 2;
const OFFER_TEST_MONEY_FOR_LEVEL_1: u32 = 35;
const OFFER_TEST_TITLE: &str = "Rest and Relaxation";
const OFFER_TEST_REWARD_TEXT: &str =
    "Rest and relaxation for the tired and cold -- that's our motto!";
const OFFER_TEST_REQUEST_ITEMS_TEXT: &str = "Have you got everything I asked for?";
const OFFER_TEST_COMPLETION_LOG: &str = "Return to Innkeeper Farley.";

#[derive(Debug, PartialEq, Eq)]
struct OfferRewardDecodedLikeCpp {
    quest_id: u32,
    auto_launched: bool,
    money: i32,
    xp: i32,
    title: String,
    reward_text: String,
}

/// Decode the leading and trailing parts of the offer-reward message. The
/// trailing string lengths sit in the 8 bit-packed bytes right before the two
/// strings (9+12+10+8+10+8 bits, `QuestPackets.cpp` `QuestGiverOfferRewardMessage`);
/// they must agree with the expected strings' lengths and the packet must end
/// exactly with them.
fn decode_offer_reward_like_cpp(
    bytes: &[u8],
    expected_title: &str,
    expected_reward_text: &str,
) -> OfferRewardDecodedLikeCpp {
    let mut pkt = WorldPacket::from_bytes(bytes);
    assert_eq!(
        pkt.server_opcode(),
        Some(wow_constants::ServerOpcodes::QuestGiverOfferRewardMessage)
    );
    pkt.skip_opcode();
    let _giver_guid = pkt.read_packed_guid().expect("giver guid");
    let _giver_creature_id = pkt.read_int32().expect("giver creature id");
    let quest_id = pkt.read_uint32().expect("quest id");
    for _ in 0..3 {
        let _ = pkt.read_uint32().expect("quest flags");
    }
    let _suggested_party_members = pkt.read_int32().expect("suggested party members");
    assert_eq!(pkt.read_int32().expect("emote count"), 0);
    let auto_launched = pkt.read_bit().expect("auto launched");
    let _unused = pkt.read_bit().expect("unused bit");
    let _choice_item_count = pkt.read_uint32().expect("choice item count");
    let _item_count = pkt.read_uint32().expect("item count");
    for _ in 0..QUEST_REWARD_ITEM_COUNT {
        let _ = pkt.read_int32().expect("reward item id");
        let _ = pkt.read_int32().expect("reward item qty");
    }
    let money = pkt.read_int32().expect("money");
    let xp = pkt.read_int32().expect("xp");

    let strings_len = expected_title.len() + expected_reward_text.len();
    assert!(bytes.len() >= strings_len + 8);
    let bits_start = bytes.len() - strings_len - 8;
    let mut tail = WorldPacket::from_bytes(&bytes[bits_start..]);
    let title_len = tail.read_bits(9).expect("title length") as usize;
    let reward_text_len = tail.read_bits(12).expect("reward text length") as usize;
    for (bits, name) in [(10, "portrait giver text"), (8, "portrait giver name")] {
        assert_eq!(tail.read_bits(bits).expect(name), 0, "{name}");
    }
    for (bits, name) in [(10, "portrait turn-in text"), (8, "portrait turn-in name")] {
        assert_eq!(tail.read_bits(bits).expect(name), 0, "{name}");
    }
    assert_eq!(title_len, expected_title.len(), "title length bits");
    assert_eq!(
        reward_text_len,
        expected_reward_text.len(),
        "reward text length bits"
    );
    let title = tail.read_string(title_len).expect("title");
    let reward_text = tail.read_string(reward_text_len).expect("reward text");
    assert_eq!(
        tail.remaining(),
        0,
        "offer reward must end with its strings"
    );

    OfferRewardDecodedLikeCpp {
        quest_id,
        auto_launched,
        money,
        xp,
        title,
        reward_text,
    }
}

/// Trailing `QuestTitle`/`CompletionText` of `SMSG_QUEST_GIVER_REQUEST_ITEMS`
/// (9+12 bits, flushed to 3 bytes, then the two strings).
fn decode_request_items_texts_like_cpp(
    bytes: &[u8],
    expected_title: &str,
    expected_completion_text: &str,
) -> (String, String) {
    assert_eq!(
        WorldPacket::from_bytes(bytes).server_opcode(),
        Some(wow_constants::ServerOpcodes::QuestGiverRequestItems)
    );
    let strings_len = expected_title.len() + expected_completion_text.len();
    assert!(bytes.len() >= strings_len + 3);
    let mut tail = WorldPacket::from_bytes(&bytes[bytes.len() - strings_len - 3..]);
    let title_len = tail.read_bits(9).expect("title length") as usize;
    let completion_text_len = tail.read_bits(12).expect("completion text length") as usize;
    assert_eq!(title_len, expected_title.len(), "title length bits");
    assert_eq!(
        completion_text_len,
        expected_completion_text.len(),
        "completion text length bits"
    );
    let title = tail.read_string(title_len).expect("title");
    let completion_text = tail
        .read_string(completion_text_len)
        .expect("completion text");
    assert_eq!(tail.remaining(), 0);
    (title, completion_text)
}

fn offer_test_quest(quest_id: u32, offer_reward_text: &str) -> QuestTemplate {
    let mut quest = quest_template(quest_id);
    quest.flags = QUEST_FLAGS_AUTO_COMPLETE_LIKE_CPP;
    quest.quest_level = 1;
    quest.reward_xp_difficulty = OFFER_TEST_XP_DIFFICULTY;
    quest.reward_money_difficulty = OFFER_TEST_MONEY_DIFFICULTY;
    quest.reward_money_multiplier = 1.0;
    quest.log_title = OFFER_TEST_TITLE.into();
    quest.quest_completion_log = OFFER_TEST_COMPLETION_LOG.into();
    quest.offer_reward_text = offer_reward_text.into();
    quest.request_items_text = OFFER_TEST_REQUEST_ITEMS_TEXT.into();
    quest
}

fn install_level_one_money_reward_store_like_cpp(session: &mut WorldSession) {
    let mut difficulty = [0; 10];
    difficulty[OFFER_TEST_MONEY_DIFFICULTY as usize] = OFFER_TEST_MONEY_FOR_LEVEL_1;
    session.set_quest_money_reward_store(Arc::new(QuestMoneyRewardStore::from_entries([
        QuestMoneyRewardEntry { id: 1, difficulty },
    ])));
}

fn insert_quest_status_like_cpp(session: &mut WorldSession, quest_id: u32, status: u8) {
    session
        .quest_state
        .fixture_insert_player_quest_status_like_cpp(
            quest_id,
            PlayerQuestStatus {
                quest_id,
                status,
                explored: false,
                accept_time_secs: 0,
                end_time_secs: 0,
                objective_counts: Vec::new(),
                slot: 0,
            },
        );
}

fn recv_quest_complete_xp_like_cpp(send_rx: &flume::Receiver<Vec<u8>>, quest_id: u32) -> u32 {
    while let Ok(bytes) = send_rx.try_recv() {
        let mut pkt = WorldPacket::from_bytes(&bytes);
        if pkt.server_opcode() != Some(wow_constants::ServerOpcodes::QuestGiverQuestComplete) {
            continue;
        }
        pkt.skip_opcode();
        assert_eq!(pkt.read_uint32().expect("quest id"), quest_id);
        return pkt.read_uint32().expect("xp reward");
    }
    panic!("QuestGiverQuestComplete was not sent");
}

#[tokio::test]
async fn quest_giver_request_reward_offer_carries_granted_xp_money_and_offer_text_like_cpp() {
    let (mut session, send_rx) = make_session();
    let player_guid = session.player_guid().expect("player guid");
    let quest_id = 2158;
    session.set_quest_store(Arc::new(QuestStore::from_quests_like_cpp([
        offer_test_quest(quest_id, OFFER_TEST_REWARD_TEXT),
    ])));
    install_level_one_money_reward_store_like_cpp(&mut session);
    insert_quest_status_like_cpp(&mut session, quest_id, QUEST_STATUS_COMPLETE_LIKE_CPP);

    session
        .handle_quest_giver_request_reward(quest_giver_request_reward_packet_like_cpp(
            player_guid,
            quest_id,
        ))
        .await;

    let offer = send_rx.try_recv().expect("offer reward packet");
    let decoded = decode_offer_reward_like_cpp(&offer, OFFER_TEST_TITLE, OFFER_TEST_REWARD_TEXT);
    assert_eq!(
        decoded,
        OfferRewardDecodedLikeCpp {
            quest_id,
            // C++ HandleQuestgiverRequestRewardOpcode (QuestHandler.cpp:436)
            // passes autoLaunched = true.
            auto_launched: true,
            // GetQuestMoneyReward: QuestMoneyReward[questLevel].Difficulty[2] * 1.0,
            // not the raw RewardMoneyDifficulty index.
            money: OFFER_TEST_MONEY_FOR_LEVEL_1 as i32,
            xp: 200,
            title: OFFER_TEST_TITLE.into(),
            reward_text: OFFER_TEST_REWARD_TEXT.into(),
        }
    );
    assert!(send_rx.try_recv().is_err());

    // The XP the dialog shows is the XP Player::RewardQuest then grants.
    session
        .handle_quest_giver_choose_reward(quest_giver_choose_reward_packet_like_cpp(
            player_guid,
            quest_id,
            QUEST_CHOICE_LOOT_ITEM_TYPE_ITEM_LIKE_CPP,
            0,
        ))
        .await;
    assert_eq!(
        recv_quest_complete_xp_like_cpp(&send_rx, quest_id),
        decoded.xp as u32
    );
}

#[tokio::test]
async fn quest_giver_request_reward_offer_without_offer_reward_row_has_empty_text_like_cpp() {
    let (mut session, send_rx) = make_session();
    let player_guid = session.player_guid().expect("player guid");
    let quest_id = 2159;
    // No `quest_offer_reward` row: `_offerRewardText` stays empty. No
    // QuestMoneyReward store either: `Quest::MoneyValue` returns 0.
    session.set_quest_store(Arc::new(QuestStore::from_quests_like_cpp([
        offer_test_quest(quest_id, ""),
    ])));
    insert_quest_status_like_cpp(&mut session, quest_id, QUEST_STATUS_COMPLETE_LIKE_CPP);

    session
        .handle_quest_giver_request_reward(quest_giver_request_reward_packet_like_cpp(
            player_guid,
            quest_id,
        ))
        .await;

    let offer = send_rx.try_recv().expect("offer reward packet");
    let decoded = decode_offer_reward_like_cpp(&offer, OFFER_TEST_TITLE, "");
    assert_eq!(decoded.reward_text, "");
    assert_eq!(decoded.money, 0);
    assert_eq!(decoded.xp, 200);
    // Neither the quest-log completion text nor the request-items text leaks
    // into RewardText.
    for other in [OFFER_TEST_COMPLETION_LOG, OFFER_TEST_REQUEST_ITEMS_TEXT] {
        assert!(
            !offer
                .windows(other.len())
                .any(|window| window == other.as_bytes()),
            "{other:?} must not be sent as RewardText"
        );
    }
}

#[tokio::test]
async fn quest_giver_complete_quest_offer_uses_player_rewards_and_offer_text_like_cpp() {
    let (mut session, send_rx) = make_session();
    let player_guid = session.player_guid().expect("player guid");
    let quest_id = 2160;
    session.set_quest_store(Arc::new(QuestStore::from_quests_like_cpp([
        offer_test_quest(quest_id, OFFER_TEST_REWARD_TEXT),
    ])));
    install_level_one_money_reward_store_like_cpp(&mut session);
    insert_quest_status_like_cpp(&mut session, quest_id, QUEST_STATUS_COMPLETE_LIKE_CPP);

    // Auto-complete quest completed from script by the player itself
    // (`fromScript` bit set), which C++ answers with the offer-reward dialog.
    session
        .handle_quest_giver_complete_quest(quest_giver_cmsg_packet(player_guid, quest_id, 0x80))
        .await;

    let offer = send_rx.try_recv().expect("offer reward packet");
    let decoded = decode_offer_reward_like_cpp(&offer, OFFER_TEST_TITLE, OFFER_TEST_REWARD_TEXT);
    assert_eq!(decoded.quest_id, quest_id);
    assert!(decoded.auto_launched);
    assert_eq!(decoded.money, OFFER_TEST_MONEY_FOR_LEVEL_1 as i32);
    assert_eq!(decoded.xp, 200);
    assert_eq!(decoded.reward_text, OFFER_TEST_REWARD_TEXT);
    assert!(send_rx.try_recv().is_err());
}

#[tokio::test]
async fn quest_giver_complete_incomplete_quest_request_items_sends_request_items_text_like_cpp() {
    let (mut session, send_rx) = make_session();
    let player_guid = session.player_guid().expect("player guid");
    let quest_id = 2161;
    let mut quest = offer_test_quest(quest_id, OFFER_TEST_REWARD_TEXT);
    // Keep the area description distinct: it is not the request-items text.
    quest.area_description = "Lion's Pride Inn".into();
    session.set_quest_store(Arc::new(QuestStore::from_quests_like_cpp([quest])));
    insert_quest_status_like_cpp(&mut session, quest_id, QUEST_STATUS_INCOMPLETE_LIKE_CPP);

    session
        .handle_quest_giver_complete_quest(quest_giver_cmsg_packet(player_guid, quest_id, 0x80))
        .await;

    let request_items = send_rx.try_recv().expect("request items packet");
    assert_eq!(
        decode_request_items_texts_like_cpp(
            &request_items,
            OFFER_TEST_TITLE,
            OFFER_TEST_REQUEST_ITEMS_TEXT
        ),
        (
            OFFER_TEST_TITLE.to_string(),
            OFFER_TEST_REQUEST_ITEMS_TEXT.to_string()
        )
    );
    assert!(send_rx.try_recv().is_err());
}
