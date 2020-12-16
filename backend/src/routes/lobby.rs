use crate::planetwars::{self, FinishedState};
use crate::util::*;
use crate::game_manager::GameManager;

use rocket::{Route, State};
use rocket_contrib::json::Json;
use rocket_contrib::templates::Template;

use async_std::fs;
use async_std::prelude::StreamExt;

use futures::executor::ThreadPool;
use futures::future::{join_all, FutureExt};

use serde_json::Value;

use rand::prelude::*;
use std::time::SystemTime;

/// The type required to build a game.
/// (json in POST request).
#[derive(Deserialize, Debug)]
pub struct GameReq {
    pub nop: u64,
    pub max_turns: u64,
    pub map: String,
    pub name: String,
}

/// Response when building a game.
#[derive(Serialize)]
struct GameRes {
    players: Vec<String>,
    state: Value,
}

/// Standard get function for the lobby tab
#[get("/lobby")]
async fn get_lobby(
    gm: State<'_, GameManager>,
    state: State<'_, Games>,
) -> Result<Template, String> {
    let maps = get_maps().await?;
    let games = get_states(&state.get_games(), &gm).await?;
    let context = Context::new_with("Lobby", Lobby { games, maps });
    Ok(Template::render("lobby", &context))
}

/// The lobby get's this automatically on load and on refresh.
#[get("/partial/state")]
async fn state_get(
    gm: State<'_, GameManager>,
    state: State<'_, Games>,
) -> Result<Template, String> {
    let games = get_states(&state.get_games(), &gm).await?;
    let context = Context::new_with(
        "Lobby",
        Lobby {
            games,
            maps: Vec::new(),
        },
    );

    Ok(Template::render("state_partial", &context))
}

/// Post function to create a game.
/// Returns the keys of the players in json.
#[post("/lobby", data = "<game_req>")]
async fn post_game(
    game_req: Json<GameReq>,
    gm: State<'_, GameManager>,
    state: State<'_, Games>,
) -> Result<Json<GameRes>, String> {
    let name = game_req.name.clone();
    let mut gm_ = gm.inner().clone();
    let tokens = gm_.create_game(game_req.into_inner());

    // TODO, I guess
    state.add_game(name, 0);

    Ok(Json(GameRes {
        players: tokens.iter().map(hex::encode).collect(),
        state: Value::Null,
    }))
}

// /// game::Manager spawns game::Builder to start games.
// /// This returns such a Builder for a planetwars game.
// fn build_builder(
//     pool: ThreadPool,
//     number_of_clients: u64,
//     max_turns: u64,
//     map: &str,
//     name: &str,
// ) -> game::Builder<planetwars::PlanetWarsGame> {
//     let config = planetwars::Config {
//         map_file: map.to_string(),
//         max_turns: max_turns,
//     };

//     let game = planetwars::PlanetWarsGame::new(
//         config.create_game(number_of_clients as usize),
//         &generate_string_id(),
//         name,
//         map,
//     );

//     let players: Vec<PlayerId> = (0..number_of_clients).collect();

//     game::Builder::new(players.clone(), game).with_step_lock(
//         StepLock::new(players.clone(), pool.clone())
//             .with_timeout(std::time::Duration::from_secs(1)),
//     )
// }

/// Fuels the lobby routes
pub fn fuel(routes: &mut Vec<Route>) {
    routes.extend(routes![post_game, get_lobby, state_get]);
}

#[derive(Serialize)]
pub struct Lobby {
    pub games: Vec<GameState>,
    pub maps: Vec<Map>,
}

#[derive(Serialize)]
pub struct Map {
    name: String,
    url: String,
}

async fn get_maps() -> Result<Vec<Map>, String> {
    let mut maps = Vec::new();
    let mut entries = fs::read_dir("maps")
        .await
        .map_err(|_| "IO error".to_string())?;
    while let Some(file) = entries.next().await {
        let file = file.map_err(|_| "IO error".to_string())?.path();
        if let Some(stem) = file.file_stem().and_then(|x| x.to_str()) {
            maps.push(Map {
                name: stem.to_string(),
                url: file.to_str().unwrap().to_string(),
            });
        }
    }

    Ok(maps)
}

pub async fn get_states(
    game_ids: &Vec<(String, u64, SystemTime)>,
    manager: &GameManager,
) -> Result<Vec<GameState>, String> {
    return Ok(Vec::new());
    // let mut states = Vec::new();
    // let gss = join_all(
    //     game_ids
    //         .iter()
    //         .cloned()
    //         .map(|(name, id, time)| manager.get_state(id).map(move |f| (f, name, time))),
    // )
    // .await;

    // for (gs, name, time) in gss {
    //     if let Some(state) = gs {
    //         match state {
    //             Ok((state, conns)) => {
    //                 let players: Vec<PlayerStatus> =
    //                     conns.iter().cloned().map(|x| x.into()).collect();
    //                 let connected = players.iter().filter(|x| x.connected).count();

    //                 states.push(GameState::Playing {
    //                     name: name,
    //                     total: players.len(),
    //                     players,
    //                     connected,
    //                     map: String::new(),
    //                     state,
    //                     time,
    //                 });
    //             }
    //             Err(value) => {
    //                 let state: FinishedState = serde_json::from_value(value).expect("Shit failed");
    //                 states.push(state.into());
    //             }
    //         }
    //     }
    // }

    // states.sort();
    // Ok(states)
}
