use crate::{message::MessageToClient, Mile, Plate, Road, Speed, Timestamp};
use std::{
    collections::{hash_map::Entry, HashMap, HashSet},
    net::SocketAddr,
};
use tokio::sync::mpsc;

pub struct State {
    dispatchers_by_road: HashMap<Road, HashMap<SocketAddr, mpsc::Sender<MessageToClient>>>,
    pending_by_road: HashMap<Road, Vec<MessageToClient>>,
    cars_seen: HashMap<(Plate, Road), Vec<(Mile, Timestamp)>>,
    ticketed_per_day: HashSet<(Timestamp, Plate)>,
}

impl State {
    pub fn new() -> Self {
        Self {
            dispatchers_by_road: HashMap::default(),
            pending_by_road: HashMap::default(),
            cars_seen: HashMap::default(),
            ticketed_per_day: HashSet::default(),
        }
    }

    pub async fn insert_dispatcher(
        &mut self,
        roads: &[Road],
        addr: SocketAddr,
        send: mpsc::Sender<MessageToClient>,
    ) {
        for road in roads {
            let Entry::Vacant(v)  = self.dispatchers_by_road
                .entry(*road)
                .or_default()
                .entry(addr) else {
                    panic!("inserting into existing addr entry")
                };

            v.insert(send.clone());

            for ticket in self.pending_by_road.entry(*road).or_default().drain(..) {
                #[cfg(debug_assertions)]
                {
                    println!("Sending pending ticket for road {road}: {ticket:?}");
                }

                send.send(ticket)
                    .await
                    .expect("received non-functioning dispatcher");
            }
        }
    }

    pub fn remove_dispatcher(&mut self, roads: &[Road], addr: SocketAddr) {
        for road in roads {
            let Entry::Occupied(o) = self
                .dispatchers_by_road
                .get_mut(&road)
                .expect("tried to remove from non-existing road").entry(addr) else {
                    panic!("removing from non-existing addr entry")
                };

            o.remove();
        }
    }

    pub async fn report_plate(
        &mut self,
        road: Road,
        mile: Mile,
        limit: Speed,
        plate: Plate,
        timestamp: Timestamp,
    ) {
        let seen = self.cars_seen.entry((plate.clone(), road)).or_default();

        for (other_mile, other_timestamp) in seen.iter() {
            let distance = if mile > *other_mile {
                mile - *other_mile
            } else {
                *other_mile - mile
            };

            let (time, mile1, timestamp1, mile2, timestamp2) = if timestamp > *other_timestamp {
                (
                    timestamp - *other_timestamp,
                    *other_mile,
                    *other_timestamp,
                    mile,
                    timestamp,
                )
            } else {
                (
                    *other_timestamp - timestamp,
                    mile,
                    timestamp,
                    *other_mile,
                    *other_timestamp,
                )
            };

            let speed = (distance as u64 * 3600 * 100 / time as u64) as u16;

            if speed > limit * 100 {
                let ticket = MessageToClient::Ticket {
                    plate: plate.clone(),
                    road,
                    mile1,
                    timestamp1,
                    mile2,
                    timestamp2,
                    speed,
                };

                let day_start = timestamp1 / 86400;
                let day_end = timestamp2 / 86400;

                let mut all_days_ticketed = true;

                for day in day_start..=day_end {
                    let is_new_ticket = !self.ticketed_per_day.contains(&(day, plate.clone()));

                    if !is_new_ticket {
                        all_days_ticketed = false;
                        continue;
                    }
                }

                if all_days_ticketed {
                    for day in day_start..=day_end {
                        self.ticketed_per_day.insert((day, plate.clone()));
                    }

                    if let Some(dispatcher) = self
                        .dispatchers_by_road
                        .get_mut(&road)
                        .and_then(|by_road| by_road.values().next())
                    {
                        dispatcher
                            .send(ticket.clone())
                            .await
                            .expect("a dispatcher send wasn't cleaned up");
                    } else {
                        #[cfg(debug_assertions)]
                        {
                            println!("Inserting pending ticket for road {road}: {ticket:?}")
                        }

                        self.pending_by_road
                            .entry(road)
                            .or_default()
                            .push(ticket.clone());
                    }
                }
            }
        }

        seen.push((mile, timestamp));
    }
}
