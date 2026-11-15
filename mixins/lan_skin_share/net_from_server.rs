fn lan_skin_share_net_from_server(
    _game: &mut crate::game::Game<'_>,
    channel: &str,
    data: &[u8],
) {
    crate::game::lan_skin_share::client_receive(channel, data);
}
