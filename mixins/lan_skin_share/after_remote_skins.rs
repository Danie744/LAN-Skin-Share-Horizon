fn lan_skin_share_after_remote_skins(game: &mut crate::game::Game<'_>, _result: ()) {
    crate::game::lan_skin_share::publish_local_skin(game);

    // This helper is compiled in remote.rs, so it can use the same private
    // atlas/Remote members as Horizon's original update_remote_skins method.
    let shared = crate::game::lan_skin_share::client_skin_snapshot();
    if shared.is_empty() {
        return;
    }

    let Some((level_players, own_uuid)) = game.remote.as_ref().map(|remote| {
        (
            remote.level_players
                .iter()
                .map(|(entity_id, uuid)| (*entity_id as u32, *uuid))
                .collect::<Vec<_>>(),
            remote.profile,
        )
    }) else {
        return;
    };

    let mut skins = game.remote.as_ref().map(|remote| remote.skins.clone()).unwrap_or_default();

    for (entity_id, uuid) in level_players {
        let Some(skin) = shared.get(&uuid) else { continue };
        let Some(rgba) = crate::game::lan_skin_share::decode_skin_png(&skin.png) else { continue };
        let location = crate::game::lan_skin_share::skin_location(uuid, &skin.png, skin.slim);

        if !game.skin_slots.contains(&location) || !game.gui.atlas.sprites.contains_key(&location) {
            game.store_skin(location.clone(), rgba);
        }
        if !game.again_skin(&location) {
            continue;
        }

        let Some(index) = game.mobs.iter().position(|mob| {
            mob.id == entity_id && &*mob.kind.id == "minecraft:player"
        }) else {
            continue;
        };
        let kind = game.mobs[index].kind.clone();
        let skin_3d = game.skin_3d_model(&kind, &location, skin.slim);
        let mob = &mut game.mobs[index];
        mob.skin = Some(location.clone());
        mob.slim = skin.slim;
        mob.skin_3d = skin_3d;
        skins.insert(uuid, location);
    }

    if let Some(skin) = shared.get(&own_uuid)
        && game.player_model.is_some()
    {
        if let Some(rgba) = crate::game::lan_skin_share::decode_skin_png(&skin.png) {
            let location = crate::game::lan_skin_share::skin_location(own_uuid, &skin.png, skin.slim);
            if !game.skin_slots.contains(&location) || !game.gui.atlas.sprites.contains_key(&location) {
                game.store_skin(location.clone(), rgba);
            }
            if game.again_skin(&location) {
                let kind = game.player_model.as_ref().map(|mob| mob.kind.clone());
                let skin_3d = kind.as_ref().and_then(|kind| game.skin_3d_model(kind, &location, skin.slim));
                if let Some(mob) = game.player_model.as_mut() {
                    mob.skin = Some(location.clone());
                    mob.slim = skin.slim;
                    mob.skin_3d = skin_3d;
                }
                skins.insert(own_uuid, location);
            }
        }
    }

    if let Some(remote) = game.remote.as_mut() {
        if remote.skins != skins {
            remote.skins = skins;
            remote.skins_revision = remote.skins_revision.wrapping_add(1);
        }
    }
}
