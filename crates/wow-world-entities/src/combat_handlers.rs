// Copyright (c) 2026 alseif0x
// RustyCore — WoW WotLK 3.4.3 server in Rust
// Licensed under GPL v3 — https://www.gnu.org/licenses/gpl-3.0.html

//! Combat packet handlers that only need the canonical combat state.
//!
//! C++ source of truth: `src/server/game/Handlers/CombatHandler.cpp` for
//! `WorldSession::HandleAttackSwingOpcode`, `WorldSession::HandleAttackStopOpcode`
//! and `WorldSession::HandleSetSheathedOpcode`. The family now owns all three
//! registrations (#1263 F5 `#1263 F5 remaining families`): attack-stop and
//! set-sheathed run on the borrowed hub context, while the attack-swing body
//! stays in the World shell — its admission seam
//! `start_player_attack_like_cpp` is shell-owned — and the host lends that
//! operation.

use tracing::{debug, warn};
use wow_constants::{ClientOpcodes, SheathState};
use wow_handler::{
    DuplicateHandlerRegistrationLikeCpp, HandlerFuture, PacketHandlerEntry, PacketProcessing,
    RegistryBuilder, SessionStatus,
};
use wow_packet::packets::combat::{SAttackStop, SetSheathed};
use wow_packet::packets::update::{UnitDataValuesDeltaUpdate, UpdateObject};
use wow_packet::{ClientPacket, ServerPacket, WorldPacket};
use wow_world_core::session::HubMut;

/// Borrowed inputs of one combat handler invocation.
pub struct CombatHandlerCxLikeCpp<'a> {
    hub: HubMut<'a>,
}

impl<'a> CombatHandlerCxLikeCpp<'a> {
    pub fn new(hub: HubMut<'a>) -> Self {
        Self { hub }
    }

    /// CMSG_ATTACK_STOP — client stops attacking.
    pub async fn handle_attack_stop(&mut self, _pkt: WorldPacket) {
        let Some(player_guid) = self.hub.shared().core.player_guid() else {
            return;
        };

        debug!(
            account = self.hub.shared().core.account_id,
            "CMSG_ATTACK_STOP"
        );

        if let Some(target) = self.hub.stop_player_attack_like_cpp() {
            crate::melee_publication::publish_attack_state(
                self.hub.shared(),
                player_guid,
                &SAttackStop {
                    attacker: player_guid,
                    victim: target,
                    now_dead: false,
                },
            );
        }
    }

    /// CMSG_SET_SHEATHED — client changes weapon sheathe state.
    ///
    /// C++ `WorldSession::HandleSetSheathedOpcode` (`CombatHandler.cpp:68-77`): a state at or
    /// above `MAX_SHEATH_STATE` is logged and ignored, any other one goes to
    /// `Unit::SetSheath` (`Unit.cpp:5605`), which writes `UnitData::SheatheState`
    /// (`UpdateField<uint8, 64, 77>`). The client draws its own weapons at once; the
    /// values update tells the players around it, and the stored state survives a relog.
    pub fn handle_set_sheathed(&mut self, mut pkt: WorldPacket) {
        let Ok(sheathed) = SetSheathed::read(&mut pkt) else {
            return;
        };
        let state = match sheathed.current_sheath_state {
            0 => SheathState::Unarmed,
            1 => SheathState::Melee,
            2 => SheathState::Ranged,
            other => {
                warn!(
                    account = self.hub.shared().core.account_id,
                    state = other,
                    "Unknown sheath state"
                );
                return;
            }
        };
        let Some(player_guid) = self.hub.shared().core.player_guid() else {
            return;
        };
        let changed = self
            .hub
            .shared()
            .core
            .mutate_canonical_player_like_cpp(|player| {
                let unit = player.unit_mut();
                let previous = unit.sheath_like_cpp();
                unit.set_sheath_like_cpp(state);
                previous != state
            })
            .unwrap_or(false);
        debug!(
            account = self.hub.shared().core.account_id,
            state = state as u8,
            changed,
            "SetSheathed"
        );
        if !changed {
            return;
        }
        // Until map-owned SendObjectUpdates has real fanout, publish this one live delta like
        // the stand-state intent does: to the player and to its movement set. The canonical
        // dirty bit stays set for canonical object-update processing.
        let mut values = UnitDataValuesDeltaUpdate::default();
        values.unit_data_mask[2] = (1 << (64 - 64)) | (1 << (77 - 64));
        values.sheathe_state = state as u8;
        let (map_id, _) = self.hub.shared().core.current_runtime_map_key_like_cpp();
        let bytes = UpdateObject::unit_values_update(player_guid, map_id, values).to_bytes();
        self.hub.shared().core.send_raw_packet(&bytes);
        self.hub
            .shared()
            .broadcast_to_movement_set_like_cpp(bytes, false);
    }
}

/// Builds a combat handler context from a host's hub.
pub trait CombatHandlerHostLikeCpp<C> {
    fn combat_handler_cx_like_cpp<'a>(&'a mut self, catalogs: &'a C) -> CombatHandlerCxLikeCpp<'a>;

    /// C++ `CombatHandler.cpp` `WorldSession::HandleAttackSwingOpcode`.
    ///
    /// The legacy registration closure did not read the catalog view, so this
    /// entry point does not carry it. The body and its shell-owned admission
    /// seam (`start_player_attack_like_cpp`) stay in the World session.
    fn handle_attack_swing<'a>(&'a mut self, pkt: WorldPacket) -> HandlerFuture<'a, ()>;
}

fn handle_attack_stop_thunk<'a, S, C>(
    session: &'a mut S,
    catalogs: &'a C,
    pkt: WorldPacket,
) -> HandlerFuture<'a, ()>
where
    S: CombatHandlerHostLikeCpp<C> + Send,
    C: Sync,
{
    Box::pin(async move {
        session
            .combat_handler_cx_like_cpp(catalogs)
            .handle_attack_stop(pkt)
            .await;
    })
}

fn handle_set_sheathed_thunk<'a, S, C>(
    session: &'a mut S,
    catalogs: &'a C,
    pkt: WorldPacket,
) -> HandlerFuture<'a, ()>
where
    S: CombatHandlerHostLikeCpp<C> + Send,
    C: Sync,
{
    Box::pin(async move {
        session
            .combat_handler_cx_like_cpp(catalogs)
            .handle_set_sheathed(pkt);
    })
}

fn handle_attack_swing_thunk<'a, S, C>(
    session: &'a mut S,
    _catalogs: &'a C,
    pkt: WorldPacket,
) -> HandlerFuture<'a, ()>
where
    S: CombatHandlerHostLikeCpp<C> + Send,
    C: Sync,
{
    Box::pin(async move { session.handle_attack_swing(pkt).await })
}

/// Registers the combat handlers on the packet registry.
pub fn register_combat_handlers_like_cpp<S, C>(
    builder: &mut RegistryBuilder<S, C>,
) -> Result<(), DuplicateHandlerRegistrationLikeCpp>
where
    S: CombatHandlerHostLikeCpp<C> + Send,
    C: Sync,
{
    builder.register(PacketHandlerEntry {
        opcode: ClientOpcodes::AttackStop,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_attack_stop",
        handler: handle_attack_stop_thunk::<S, C>,
    })?;
    builder.register(PacketHandlerEntry {
        opcode: ClientOpcodes::SetSheathed,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_set_sheathed",
        handler: handle_set_sheathed_thunk::<S, C>,
    })?;
    builder.register(PacketHandlerEntry {
        opcode: ClientOpcodes::AttackSwing,
        status: SessionStatus::LoggedIn,
        processing: PacketProcessing::Inplace,
        handler_name: "handle_attack_swing",
        handler: handle_attack_swing_thunk::<S, C>,
    })?;
    Ok(())
}
