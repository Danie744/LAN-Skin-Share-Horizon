fn lan_skin_share_net_from_client(
    game: &mut crate::game::Game<'_>,
    player: u128,
    channel: &str,
    data: &[u8],
) {
    crate::game::lan_skin_share::server_receive(game, player, channel, data);
}
