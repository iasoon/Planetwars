use mozaic_core::client_manager::{ClientMgrHandle, ClientHandle};
use mozaic_core::connection_table::{Token, ConnectionTableHandle, ConnectionTable};
use mozaic_core::match_context::MatchCtx;
use mozaic_core::websocket::websocket_server;

use crate::planetwars::{self, PlanetWarsGame};
use crate::routes::GameReq;
use rand::Rng;
use tokio::runtime::Handle as RuntimeHandle;

// The Grand Poohab
// TODO: robbe fix it
#[derive(Clone)]
pub struct GameManager {
    rt_handle: RuntimeHandle,
    client_mgr: ClientMgrHandle,
    conn_table: ConnectionTableHandle,
}

impl GameManager {
    pub fn create(rt_handle: RuntimeHandle) -> Self {
        let conn_table = ConnectionTable::new();
        let client_mgr = ClientMgrHandle::new();
        return GameManager { client_mgr, conn_table, rt_handle };
    }

    pub fn start_ws_server(&self, ws_addr: &'static str) {
        self.rt_handle.spawn(websocket_server(
            self.conn_table.clone(),
            self.client_mgr.clone(),
            ws_addr
        ));
    }

    pub fn create_game(&mut self, game_req: GameReq) -> Vec<Token> {     
        // TODO: tokens should not be generated here, but passed in by the session manager.   
        let client_tokens =(0..game_req.nop).map(|_| {
            rand::thread_rng().gen()
        }).collect::<Vec<Token>>();

        let clients = client_tokens.iter().map(|token| {
            self.client_mgr.get_client(&token)
        }).collect();

        self.rt_handle.spawn(run_lobby(clients, self.conn_table.clone(), game_req));

        return client_tokens;
    }
}

async fn run_lobby(
    mut clients: Vec<ClientHandle>,
    conn_table: ConnectionTableHandle,
    game_req: GameReq)
    -> serde_json::Value
{

    let mut match_ctx = MatchCtx::new(conn_table);
    for (player_num, client) in clients.iter_mut().enumerate() {
        let player_token: Token = rand::thread_rng().gen();
        match_ctx.create_player((player_num + 1) as u32, player_token);
        client.run_player(player_token).await;
    }

    let config = planetwars::Config {
        map_file: game_req.map.clone(),
        max_turns: game_req.max_turns,
    };

    let pw_game = PlanetWarsGame::new(
        match_ctx,
        config.create_game(clients.len()),
        &generate_string_id(),
        &game_req.name,
        &game_req.map,
    );

    pw_game.run().await
}

// TODO: do we still need this?
/// Generate random ID for the game, used as filename
fn generate_string_id() -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(15)
        .collect::<String>()
        + ".json"
}
