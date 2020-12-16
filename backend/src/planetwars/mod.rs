use mozaic_core::match_context::{MatchCtx, RequestResult};
use tokio::time::Duration;
use futures::stream::futures_unordered::FuturesUnordered;
use futures::{FutureExt, StreamExt};

use serde_json;

use std::collections::HashMap;
use std::convert::TryInto;
use std::fs::{create_dir, File};
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;
use serde_json::Value;


mod pw_config;
mod pw_protocol;
mod pw_rules;
mod pw_serializer;
pub use pw_config::{Config, Map};
use pw_protocol::{self as proto, CommandError};
use pw_rules::Dispatch;

// TODO: rename game -> match
pub struct PlanetWarsGame {
    match_ctx: MatchCtx,
    state: pw_rules::PlanetWars,
    planet_map: HashMap<String, usize>,
    turns: u64,
    name: String,
    map: String,
}

impl PlanetWarsGame {
    pub fn new(match_ctx: MatchCtx, state: pw_rules::PlanetWars, location: &str, name: &str, map: &str) -> Self {
        let planet_map = state
            .planets
            .iter()
            .map(|p| (p.name.clone(), p.id))
            .collect();

        if let Err(_) = create_dir("games") {
            println!("'games' already exists");
        }
        // TODO: don't do this. yuck!
        let file = File::create(format!("games/{}", location)).unwrap();

        Self {
            state,
            planet_map,
            // log_file_loc:location.to_string(),
            // log_file: file ,
            turns: 0,
            name: name.to_string(),
            map: PathBuf::from(map)
                .file_stem()
                .and_then(|x| x.to_str())
                .unwrap()
                .to_string(),
            match_ctx,
        }
    }

    pub async fn run(mut self) -> Value {
        while !self.state.is_finished() {
            let player_messages = self.prompt_players().await;

            self.state.repopulate();
            for (player_id, turn) in player_messages {
                self.execute_action(player_id, turn);
            }
            self.state.step();

            // Log state
            let state = pw_serializer::serialize(&self.state);
            self.match_ctx.emit(serde_json::to_string(&state).unwrap());
        }

        // TODO: why is this
        json!({
            "winners": self.state.living_players(),
            "turns": self.state.turn_num,
            "name": self.name,
            "map": self.map,
            "time": SystemTime::now(),
        })
    }


    async fn prompt_players(&mut self) -> Vec<(usize, RequestResult<Vec<u8>>)> {
        // borrow these outside closure to make the borrow checker happy
        let state = &self.state;
        let match_ctx = &mut self.match_ctx;

        self.state.players.iter().filter(|p| p.alive).map(move |player| {
            let state_for_player =
                pw_serializer::serialize_rotated(&state, player.id);
            match_ctx.request(
                player.id.try_into().unwrap(),
                serde_json::to_vec(&state_for_player).unwrap(),
                Duration::from_millis(1000),
            ).map(move |resp| {
                (player.id, resp)
            })
        })
        .collect::<FuturesUnordered<_>>()
        .collect::<Vec<_>>()
        .await
    }

    fn execute_action<'a>(&mut self, player_num: usize, turn: RequestResult<Vec<u8>>) -> proto::PlayerAction {
        let turn = match turn {
            Err(_timeout) => return proto::PlayerAction::Timeout,
            Ok(data) => data,
        };

        let action: proto::Action = match serde_json::from_slice(&turn) {
            Err(err) => return proto::PlayerAction::ParseError(err.to_string()),
            Ok(action) => action,
        };

        let commands = action
            .commands
            .into_iter()
            .map(
                |command| match self.check_valid_command(player_num, &command) {
                    Ok(dispatch) => {
                        self.state.dispatch(&dispatch);
                        proto::PlayerCommand {
                            command,
                            error: None,
                        }
                    }
                    Err(error) => proto::PlayerCommand {
                        command,
                        error: Some(error),
                    },
                },
            )
            .collect();

        return proto::PlayerAction::Commands(commands);
    }

    fn check_valid_command(
        &self,
        player_num: usize,
        mv: &proto::Command,
    ) -> Result<Dispatch, CommandError> {
        let origin_id = *self
            .planet_map
            .get(&mv.origin)
            .ok_or(CommandError::OriginDoesNotExist)?;

        let target_id = *self
            .planet_map
            .get(&mv.destination)
            .ok_or(CommandError::DestinationDoesNotExist)?;

        if self.state.planets[origin_id].owner() != Some(player_num) {
            return Err(CommandError::OriginNotOwned);
        }

        if self.state.planets[origin_id].ship_count() < mv.ship_count {
            return Err(CommandError::NotEnoughShips);
        }

        if mv.ship_count == 0 {
            return Err(CommandError::ZeroShipMove);
        }

        Ok(Dispatch {
            origin: origin_id,
            target: target_id,
            ship_count: mv.ship_count,
        })
    }
}

fn get_epoch() -> SystemTime {
    SystemTime::UNIX_EPOCH
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FinishedState {
    pub winners: Vec<u64>,
    pub turns: u64,
    pub name: String,
    pub file: String,
    pub map: String,
    #[serde(default = "get_epoch")]
    pub time: SystemTime,
    pub players: Vec<(u64, String)>,
}
