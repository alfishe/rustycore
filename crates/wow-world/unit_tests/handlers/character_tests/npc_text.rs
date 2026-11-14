//! `CMSG_QUERY_NPC_TEXT` against the immutable `npc_text` catalog
//! (C++ `HandleNpcTextQueryOpcode`, QueryHandler.cpp:175-199).

use super::*;
use wow_packet::packets::gossip::{QueryNpcText, QueryNpcTextResponse};

fn install_npc_texts_like_cpp(session: &mut WorldSession, rows: Vec<wow_data::NpcTextRowLikeCpp>) {
    session.set_object_mgr_catalogs_like_cpp(Arc::new(crate::session::ObjectMgrCatalogsLikeCpp {
        npc_text: Arc::new(wow_data::NpcTextCatalogLikeCpp::from_rows_like_cpp(
            rows,
            |_| true,
        )),
        ..Default::default()
    }));
}

#[tokio::test]
async fn query_npc_text_sends_the_catalog_options_like_cpp() {
    let (mut session, send_rx) = make_session_with_send_capacity(1);
    install_npc_texts_like_cpp(
        &mut session,
        vec![wow_data::NpcTextRowLikeCpp {
            id: 1,
            probabilities: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            broadcast_text_ids: [50429, 0, 0, 0, 0, 0, 0, 0],
        }],
    );

    session
        .handle_query_npc_text(QueryNpcText {
            text_id: 1,
            guid: ObjectGuid::EMPTY,
        })
        .await;

    assert_eq!(
        send_rx.try_recv().unwrap(),
        QueryNpcTextResponse {
            text_id: 1,
            allow: true,
            probabilities: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            broadcast_text_ids: [50429, 0, 0, 0, 0, 0, 0, 0],
        }
        .to_bytes()
    );
}

#[tokio::test]
async fn query_npc_text_unknown_id_is_not_allowed_like_cpp() {
    let (mut session, send_rx) = make_session_with_send_capacity(1);
    install_npc_texts_like_cpp(&mut session, Vec::new());

    session
        .handle_query_npc_text(QueryNpcText {
            text_id: 77,
            guid: ObjectGuid::EMPTY,
        })
        .await;

    assert_eq!(
        send_rx.try_recv().unwrap(),
        QueryNpcTextResponse::not_found(77).to_bytes()
    );
}
