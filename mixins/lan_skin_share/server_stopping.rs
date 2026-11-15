fn lan_skin_share_server_stopping(_dedicated: bool) {
    crate::game::lan_skin_share::clear_server_session();
}
