//! Admin commands typed in chat, starting with `::` (crate::roles says who may use them).
//!
//! Three kinds, by who carries them out:
//! - On your own computer: ::help, ::max, ::tp, ::god, ::speed, ::item search, and ::give or
//!   ::reset aimed at yourself.
//! - On the host, for the whole session: ::bring, ::kick, ::mute, ::unmute, ::spawn bot, ::time,
//!   ::weather, ::announce. A player's command goes to the host. The host checks the sender's
//!   role against its own list, using the Steam ID it sees on the connection.
//! - Straight to the player, over Steam's authenticated messages: ::give, ::giveall, ::reset,
//!   ::admin, ::unadmin. The receiving computer applies one only if its own owners file gives
//!   the sender that role. The host never carries these.
//!
//! Every command is logged, allowed or not (admin_log.txt in the save folder). ::max, ::god,
//! ::speed and ::give to yourself mark this session: from then on it counts for no leaderboard,
//! record, career progress or achievement.

use crate::game::*;
use crate::hub::HubEv;
use crate::items::{self, ItemKey, ItemStore};
use crate::math::*;
use crate::roles::{Role, TIMES, WEATHERS};

/// One command: its name, what it takes, what it does, the role it needs.
pub struct CmdInfo {
    pub name: &'static str,
    pub args: &'static str,
    pub help: &'static str,
    pub role: Role,
}

const fn ci(name: &'static str, args: &'static str, help: &'static str, role: Role) -> CmdInfo {
    CmdInfo { name, args, help, role }
}

pub const COMMANDS: &[CmdInfo] = &[
    ci("help", "", "this list", Role::Admin),
    ci("max", "", "unlock every cosmetic, dye, title, gear look, vault drop and mastery for yourself", Role::Admin),
    ci("give", "<player> <item> [amount]", "give a player an item (made new)", Role::Admin),
    ci("giveall", "<item>", "give everyone here an item", Role::Admin),
    ci("tp", "<player>", "go to a player", Role::Admin),
    ci("bring", "<player>", "bring a player to you", Role::Admin),
    ci("kick", "<player> [reason]", "remove a player from the session", Role::Admin),
    ci("mute", "<player> [minutes]", "stop a player's chat (10 minutes unless said)", Role::Admin),
    ci("unmute", "<player>", "let them chat again", Role::Admin),
    ci("spawn", "bot [n]", "bring in n regulars (bots)", Role::Admin),
    ci("god", "", "take no damage (again: off)", Role::Admin),
    ci("speed", "<x>", "move x times as fast (1 is normal)", Role::Admin),
    ci("time", "day|dusk|night", "the hub's time of day", Role::Admin),
    ci("weather", "clear|rain|fog", "the hub's weather", Role::Admin),
    ci("announce", "<text>", "a big banner for everyone", Role::Admin),
    ci("reset", "<player>", "reset a player's progress (asks to confirm)", Role::Admin),
    ci("item", "search <name>", "find an item's exact name", Role::Admin),
    ci("admin", "<player>", "make a player an admin (owner only)", Role::Owner),
    ci("unadmin", "<player>", "take a player's admin away (owner only)", Role::Owner),
];

pub fn info(name: &str) -> Option<&'static CmdInfo> {
    COMMANDS.iter().find(|c| c.name == name)
}

/// The ::help list, a line per command (the ones `role` may use).
pub fn help_lines(role: Role) -> Vec<String> {
    COMMANDS.iter().filter(|c| c.role <= role).map(|c| format!("::{} {} - {}", c.name, c.args, c.help).replace("  ", " ")).collect()
}

/// Whether a chat line is a command.
pub fn is_command(text: &str) -> bool {
    text.trim_start().starts_with("::")
}

// ------------------------------------------------------------------ finding players and items

/// A player by name: "me", an exact name (any case), else the only one starting with it, else
/// the only one containing it.
pub fn find_player(g: &Game, q: &str) -> Result<usize, String> {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return Err("which player?".into());
    }
    if q == "me" || q == "self" {
        return Ok(g.local);
    }
    let here: Vec<usize> = (0..g.f.len()).filter(|&s| g.f[s].present && !g.f[s].hub.townsfolk).collect();
    if let Some(&s) = here.iter().find(|&&s| g.f[s].name.to_lowercase() == q) {
        return Ok(s);
    }
    for test in [|n: &str, q: &str| n.starts_with(q), |n: &str, q: &str| n.contains(q)] {
        let m: Vec<usize> = here.iter().copied().filter(|&s| test(&g.f[s].name.to_lowercase(), &q)).collect();
        match m.len() {
            1 => return Ok(m[0]),
            0 => {}
            _ => return Err(format!("\"{q}\" could be {}", m.iter().map(|&s| g.f[s].name.clone()).collect::<Vec<_>>().join(", "))),
        }
    }
    Err(format!("nobody here called \"{q}\""))
}

/// Tab: complete the command, the player or the item being typed. Returns the new text.
pub fn complete(g: &Game, text: &str) -> Option<String> {
    let t = text.trim_start();
    let rest = t.strip_prefix("::")?;
    let mut parts: Vec<&str> = rest.split(' ').collect();
    if parts.len() == 1 {
        let role = g.role_of(g.local);
        let m: Vec<&str> = COMMANDS.iter().filter(|c| c.role <= role && c.name.starts_with(parts[0])).map(|c| c.name).collect();
        return match m.len() {
            0 => None,
            1 => Some(format!("::{} ", m[0])),
            _ => Some(format!("::{}", common_prefix(&m))),
        };
    }
    let cmd = parts[0];
    let player_arg = matches!(cmd, "give" | "tp" | "bring" | "kick" | "mute" | "unmute" | "reset" | "admin" | "unadmin") && parts.len() == 2;
    if player_arg {
        let p = parts[1].to_lowercase();
        let names: Vec<String> = (0..g.f.len()).filter(|&s| g.f[s].present && !g.f[s].hub.townsfolk && g.f[s].name.to_lowercase().starts_with(&p)).map(|s| g.f[s].name.clone()).collect();
        let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        return match refs.len() {
            0 => None,
            1 => {
                parts[1] = refs[0];
                Some(format!("::{} ", parts.join(" ")))
            }
            _ => {
                let c = common_prefix(&refs);
                parts[1] = &c;
                Some(format!("::{}", parts.join(" ")))
            }
        };
    }
    // an item's name: the words after the command (and the player, for ::give)
    let skip = match cmd {
        "give" => 2,
        "giveall" => 1,
        "item" => 2,
        "time" | "weather" => 1,
        _ => return None,
    };
    if parts.len() <= skip {
        return None;
    }
    let typed = parts[skip..].join(" ").to_lowercase();
    let cands: Vec<String> = match cmd {
        "time" => TIMES.iter().map(|s| s.to_string()).collect(),
        "weather" => WEATHERS.iter().map(|s| s.to_string()).collect(),
        _ => items::catalogue().into_iter().map(|k| k.name()).collect(),
    };
    let m: Vec<&String> = cands.iter().filter(|n| n.to_lowercase().starts_with(&typed)).collect();
    let head = parts[..skip].join(" ");
    match m.len() {
        0 => None,
        1 => Some(format!("::{head} {}", m[0])),
        _ => {
            let refs: Vec<&str> = m.iter().map(|s| s.as_str()).collect();
            Some(format!("::{head} {}", common_prefix(&refs)))
        }
    }
}

fn common_prefix(v: &[&str]) -> String {
    let first = v[0];
    let mut n = first.len();
    for s in &v[1..] {
        n = n.min(first.chars().zip(s.chars()).take_while(|(a, b)| a.to_lowercase().eq(b.to_lowercase())).count());
    }
    first.chars().take(n).collect()
}

/// Possible completions for the hint under the chat box.
pub fn suggestions(g: &Game, text: &str) -> Vec<String> {
    let Some(rest) = text.trim_start().strip_prefix("::") else { return Vec::new() };
    if rest.contains(' ') {
        let cmd = rest.split(' ').next().unwrap_or("");
        return info(cmd).map(|c| vec![format!("::{} {}  -  {}", c.name, c.args, c.help)]).unwrap_or_default();
    }
    let role = g.role_of(g.local);
    COMMANDS.iter().filter(|c| c.role <= role && c.name.starts_with(rest)).take(6).map(|c| format!("::{} {}", c.name, c.args)).collect()
}

// ------------------------------------------------------------------------------ running them

fn reply(g: &mut Game, s: impl Into<String>) {
    g.admin.replies.push(s.into());
}

/// Someone (the host's view: `from`) gets an answer.
fn answer(g: &mut Game, to: usize, text: String) {
    if to == g.local {
        g.admin.replies.push(text);
    } else if g.kind == Kind::Host {
        g.out_events.push(Ev::Hub(HubEv::Reply { to: to as u8, text }));
    }
}

/// A line typed in chat that starts with "::": run it (or send it on). This is the only way in.
pub fn run(g: &mut Game, line: &str) {
    let text = line.trim().trim_start_matches("::").trim().to_string();
    let me = g.local;
    let name = g.f[me].name.clone();
    let my_id = g.admin.my_id;
    let role = g.role_of(me);
    let cmd = text.split_whitespace().next().unwrap_or("").to_lowercase();
    let Some(ci) = info(&cmd) else {
        g.admin.log_line(&name, my_id, line, "unknown");
        reply(g, format!("No command \"::{cmd}\". ::help lists them."));
        return;
    };
    if role < ci.role {
        g.admin.log_line(&name, my_id, line, "refused: not allowed");
        reply(g, if ci.role == Role::Owner { "Only the owner can use that." } else { "Only the owner and admins can use :: commands." });
        return;
    }
    g.admin.log_line(&name, my_id, line, "run");
    let args: Vec<String> = text.split_whitespace().skip(1).map(|s| s.to_string()).collect();
    match cmd.as_str() {
        "help" => {
            g.admin.show_help = true;
            reply(g, "The command list is open (Esc closes it).");
        }
        "max" => {
            g.admin.max_req = true;
            grant_everything(g);
            g.gate_session("::max");
            reply(g, "Everything is unlocked for you.");
        }
        "god" => {
            let on = !g.admin.is_god(me);
            g.admin.set_god(me, on);
            if on {
                g.gate_session("::god");
            }
            if g.kind == Kind::Client {
                g.out_events.push(Ev::Hub(HubEv::God { who: me as u8, on }));
            }
            reply(g, if on { "God mode on: you take no damage." } else { "God mode off." });
        }
        "speed" => match args.first().and_then(|a| a.parse::<f32>().ok()).filter(|x| x.is_finite()) {
            Some(x) => {
                let x = x.clamp(0.25, 4.0);
                g.admin.speed[me] = x;
                if (x - 1.0).abs() > 0.01 {
                    g.gate_session("::speed");
                }
                reply(g, format!("Speed x{x:.2}."));
            }
            None => reply(g, "::speed <x>, for example ::speed 2"),
        },
        "tp" => match args.first().map(|a| find_player(g, a)) {
            Some(Ok(t)) if t != me => {
                let p = g.f[t].pos + g.f[t].fwd() * -1.2;
                move_me(g, p);
                reply(g, format!("You go to {}.", g.f[t].name));
            }
            Some(Ok(_)) => reply(g, "That's you."),
            Some(Err(e)) => reply(g, e),
            None => reply(g, "::tp <player>"),
        },
        "item" => {
            let q = args.iter().skip(if args.first().is_some_and(|a| a == "search") { 1 } else { 0 }).cloned().collect::<Vec<_>>().join(" ");
            let found = items::search(&q);
            if found.is_empty() {
                reply(g, format!("No items match \"{q}\"."));
            } else {
                reply(g, format!("{} items match \"{q}\":", found.len()));
                for k in found.iter().take(6) {
                    reply(g, format!("  {}  [{}]  {}", k.name(), k.to_code(), k.category().name()));
                }
            }
        }
        "give" => {
            if args.len() < 2 {
                reply(g, "::give <player> <item> [amount]");
                return;
            }
            let target = match find_player(g, &args[0]) {
                Ok(t) => t,
                Err(e) => return reply(g, e),
            };
            let (item_words, n) = split_amount(&args[1..]);
            match items::find(&item_words) {
                Ok(k) => give_to(g, target, k, n),
                Err(e) => reply(g, e),
            }
        }
        "giveall" => {
            let (item_words, n) = split_amount(&args);
            match items::find(&item_words) {
                Ok(k) => {
                    let all: Vec<usize> = (0..g.f.len()).filter(|&s| g.f[s].present && !g.f[s].is_bot && !g.f[s].hub.townsfolk).collect();
                    for s in all {
                        give_to(g, s, k, n);
                    }
                }
                Err(e) => reply(g, e),
            }
        }
        "reset" => {
            let Some(a) = args.first() else { return reply(g, "::reset <player>") };
            let target = match find_player(g, a) {
                Ok(t) => t,
                Err(e) => return reply(g, e),
            };
            let confirmed = args.get(1).is_some_and(|c| c == "confirm") && g.admin.pending_reset.is_some_and(|p| p.0 == target);
            if !confirmed {
                g.admin.pending_reset = Some((target, 20.0));
                reply(g, format!("This wipes {}'s hub progress and bank. Type ::reset {} confirm within 20 s to go ahead.", g.f[target].name, g.f[target].name));
                return;
            }
            g.admin.pending_reset = None;
            if target == me {
                g.admin.reset_req = true;
                reply(g, "Your progress is reset.");
            } else if let Some(id) = steam_id(g, target) {
                g.admin.direct_out.push((id, Direct::Reset.to_bytes()));
                reply(g, format!("Reset sent to {}.", g.f[target].name));
            } else {
                reply(g, format!("Can't reach {} directly (it needs Steam).", g.f[target].name));
            }
        }
        "admin" | "unadmin" => {
            let on = cmd == "admin";
            let Some(a) = args.first() else { return reply(g, format!("::{cmd} <player>")) };
            let target = match find_player(g, a) {
                Ok(t) => t,
                Err(e) => return reply(g, e),
            };
            let Some(id) = steam_id(g, target) else { return reply(g, format!("{} has no Steam ID we know.", g.f[target].name)) };
            g.admin.roster.set_admin(id, on);
            g.admin.roster.save();
            // everyone here updates their own list (each checks it's the owner asking)
            for s in 0..g.f.len() {
                if s != me && g.f[s].present && !g.f[s].is_bot {
                    if let Some(to) = steam_id(g, s) {
                        g.admin.direct_out.push((to, Direct::Roster { id, on }.to_bytes()));
                    }
                }
            }
            reply(g, format!("{} is {} an admin.", g.f[target].name, if on { "now" } else { "no longer" }));
        }
        // the rest happen on the host
        _ => {
            if g.is_authority() {
                host_exec(g, me, &cmd, &args);
            } else {
                g.out_events.push(Ev::Hub(HubEv::Cmd { who: me as u8, text: text.clone() }));
            }
        }
    }
}

fn split_amount(args: &[String]) -> (String, u32) {
    match args.last().and_then(|a| a.parse::<u32>().ok()) {
        Some(n) if args.len() > 1 => (args[..args.len() - 1].join(" "), n.clamp(1, 99)),
        _ => (args.join(" "), 1),
    }
}

/// A slot's Steam ID if we have one (for messages straight to them).
fn steam_id(g: &Game, s: usize) -> Option<u64> {
    let id = g.admin.ids.get(s).copied().unwrap_or(0);
    (id != 0).then_some(id)
}

fn move_me(g: &mut Game, p: V3) {
    let me = g.local;
    let f = &mut g.f[me];
    f.pos = p;
    f.prev_pos = p;
    f.vel = V3::ZERO;
    f.hub.seat = crate::hub::SEAT_NONE;
}

/// Give `n` of an item to a slot: our own bank, or straight to them over Steam.
fn give_to(g: &mut Game, target: usize, k: ItemKey, n: u32) {
    if target == g.local {
        if let Some(h) = g.hub.as_mut() {
            h.bank.grant(k, n, "::give");
            h.bank.apply_to(&mut h.earned);
        }
        g.gate_session("::give to yourself");
        reply(g, format!("You get {n} {}.", k.name()));
        return;
    }
    if g.f[target].is_bot {
        return reply(g, format!("{} is a regular (a bot): no bank.", g.f[target].name));
    }
    match steam_id(g, target) {
        Some(id) => {
            g.admin.direct_out.push((id, Direct::Grant { item: k, n }.to_bytes()));
            reply(g, format!("{} gets {n} {}.", g.f[target].name, k.name()));
        }
        None => reply(g, format!("Can't reach {} directly (it needs Steam).", g.f[target].name)),
    }
}

/// ::max: every item into the bank (one each).
fn grant_everything(g: &mut Game) {
    let Some(h) = g.hub.as_mut() else { return };
    h.earned.rooftop = true;
    h.earned.table_hard = true;
    h.earned.pit_wins = h.earned.pit_wins.max(99);
    h.earned.mastery = h.earned.mastery.max(99);
    for l in 0..crate::raid::LOOT.len() as u8 {
        h.earned.raid.give(l);
    }
    h.earned.raid.cleared = true;
    h.earned.raid.cleared_hard = true;
    let e = h.earned;
    h.bank.sync(&e);
    for k in items::catalogue() {
        if !k.starter() && h.bank.count(k) == 0 {
            h.bank.grant(k, 1, "::max");
        }
    }
    h.bank.apply_to(&mut h.earned);
}

// ------------------------------------------------------------------------------- the host

/// The host carries out a session command for `issuer` (already checked).
pub fn host_exec(g: &mut Game, issuer: usize, cmd: &str, args: &[String]) {
    let target = |g: &Game| args.first().map(|a| find_player(g, a));
    match cmd {
        "bring" => match target(g) {
            Some(Ok(t)) => {
                let p = g.f[issuer].pos + g.f[issuer].fwd() * 1.5;
                send_move(g, t, p);
                answer(g, issuer, format!("{} is brought to you.", g.f[t].name));
            }
            Some(Err(e)) => answer(g, issuer, e),
            None => answer(g, issuer, "::bring <player>".into()),
        },
        "kick" => match target(g) {
            Some(Ok(t)) if t == g.local || t == issuer => answer(g, issuer, "You can't kick that player.".into()),
            Some(Ok(t)) => {
                let reason = if args.len() > 1 { args[1..].join(" ") } else { "removed by an admin".into() };
                let nm = g.f[t].name.clone();
                if g.f[t].is_bot {
                    g.remove_player(t);
                } else {
                    g.out_events.push(Ev::Hub(HubEv::Kicked { who: t as u8, reason: reason.clone() }));
                    g.admin.kicked_slots.push(t);
                }
                g.hub_broadcast(HubEv::Note { text: format!("{nm} was removed ({reason})") });
                answer(g, issuer, format!("{nm} is removed."));
            }
            Some(Err(e)) => answer(g, issuer, e),
            None => answer(g, issuer, "::kick <player> [reason]".into()),
        },
        "mute" | "unmute" => match target(g) {
            Some(Ok(t)) => {
                let mins = if cmd == "mute" { args.get(1).and_then(|a| a.parse::<f32>().ok()).unwrap_or(10.0).clamp(0.5, 600.0) } else { 0.0 };
                let secs = (mins * 60.0) as u16;
                g.hub_broadcast(HubEv::Mute { who: t as u8, secs });
                answer(g, issuer, if secs > 0 { format!("{} is muted for {mins:.0} minutes.", g.f[t].name) } else { format!("{} can chat again.", g.f[t].name) });
            }
            Some(Err(e)) => answer(g, issuer, e),
            None => answer(g, issuer, format!("::{cmd} <player>")),
        },
        "spawn" => {
            let n = args.iter().find_map(|a| a.parse::<usize>().ok()).unwrap_or(1).clamp(1, 8);
            let mut added = 0;
            for _ in 0..n {
                if let Some(s) = (1..g.f.len().min(crate::hub::HUB_PLAYERS)).find(|&s| !g.f[s].present) {
                    g.add_regular(s);
                    added += 1;
                }
            }
            answer(g, issuer, format!("{added} regular{} came in.", if added == 1 { "" } else { "s" }));
        }
        "time" | "weather" => {
            let list: &[&str] = if cmd == "time" { &TIMES } else { &WEATHERS };
            match args.first().and_then(|a| list.iter().position(|t| *t == a.to_lowercase())) {
                Some(v) => {
                    let (mut time, mut weather) = (g.admin.time_of_day, g.admin.weather);
                    if cmd == "time" {
                        time = v as u8;
                    } else {
                        weather = v as u8;
                    }
                    g.hub_broadcast(HubEv::World { time, weather });
                    answer(g, issuer, format!("The {cmd} is now {}.", list[v]));
                }
                None => answer(g, issuer, format!("::{cmd} {}", list.join("|"))),
            }
        }
        "announce" => {
            let text: String = args.join(" ").chars().take(120).collect();
            if text.is_empty() {
                answer(g, issuer, "::announce <text>".into());
            } else {
                g.hub_broadcast(HubEv::Announce { text });
            }
        }
        _ => answer(g, issuer, format!("::{cmd} isn't carried out by the host.")),
    }
}

fn send_move(g: &mut Game, t: usize, p: V3) {
    if t == g.local {
        move_me(g, p);
    } else if g.f[t].is_bot {
        let f = &mut g.f[t];
        f.pos = p;
        f.prev_pos = p;
    } else {
        g.out_events.push(Ev::Hub(HubEv::Move { who: t as u8, pos: p }));
    }
}

/// The admin events (crate::hub::HubEv): `from` is who sent it as the host sees it, or the host
/// as a player sees it.
pub fn on_hub(g: &mut Game, from: usize, e: HubEv) {
    let auth = g.is_authority();
    // what only the host may say: a player's own copy of it is ignored
    let from_host = !auth || from == g.local;
    let me = g.local;
    match e {
        HubEv::Cmd { text, .. } if auth => {
            let role = g.role_of(from);
            let name = g.f.get(from).map(|f| f.name.clone()).unwrap_or_default();
            let id = g.admin.ids.get(from).copied().unwrap_or(0);
            let cmd = text.split_whitespace().next().unwrap_or("").to_lowercase();
            let need = info(&cmd).map_or(Role::Owner, |c| c.role);
            if role < need || role < Role::Admin {
                g.admin.log_line(&name, id, &format!("::{text}"), "refused by the host: not allowed");
                answer(g, from, "The host doesn't see you as an admin.".into());
                return;
            }
            g.admin.log_line(&name, id, &format!("::{text}"), "run by the host");
            let args: Vec<String> = text.split_whitespace().skip(1).map(|s| s.to_string()).collect();
            host_exec(g, from, &cmd, &args);
        }
        HubEv::Reply { to, text } if from_host => {
            if to as usize == me {
                g.admin.replies.push(text);
            }
        }
        HubEv::Ids { list } if !auth => g.set_reported_ids(&list),
        HubEv::Announce { text } if from_host => g.admin.announce = Some((text, 6.0)),
        HubEv::World { time, weather } if from_host => {
            g.admin.time_of_day = time.min(2);
            g.admin.weather = weather.min(2);
        }
        HubEv::Move { who, pos } if from_host => {
            if who as usize == me {
                move_me(g, pos);
            }
        }
        HubEv::Mute { who, secs } if from_host => {
            let w = who as usize;
            if w < MAX_PLAYERS {
                g.admin.muted[w] = if secs == 0 { 0.0 } else { g.time + secs as f32 };
                if w == me {
                    g.admin.replies.push(if secs == 0 { "You can chat again.".into() } else { format!("An admin muted you for {} minutes.", (secs as f32 / 60.0).round()) });
                }
            }
        }
        HubEv::Kicked { who, reason } if !auth => {
            if who as usize == me {
                g.admin.kicked = Some(reason);
            }
        }
        HubEv::God { on, .. } if auth => g.admin.set_god(from, on),
        HubEv::Gated { .. } if auth => {
            if from < 32 {
                g.admin.gated_slots |= 1 << from;
            }
        }
        _ => {}
    }
}

/// Whether a slot may chat (not muted).
pub fn may_chat(g: &Game, slot: usize) -> bool {
    slot >= MAX_PLAYERS || g.admin.muted[slot] <= g.time
}

/// (host) The players' Steam IDs for everyone (every few seconds).
pub fn host_ids(g: &Game) -> HubEv {
    let list = (0..g.f.len().min(MAX_PLAYERS)).filter(|&s| g.f[s].present && !g.f[s].is_bot).map(|s| (s as u8, g.admin.ids[s])).collect();
    HubEv::Ids { list }
}

/// Each frame: clocks.
pub fn tick(g: &mut Game, dt: f32) {
    if let Some((_, t)) = g.admin.announce.as_mut() {
        *t -= dt;
        if *t <= 0.0 {
            g.admin.announce = None;
        }
    }
    if let Some((_, t)) = g.admin.pending_reset.as_mut() {
        *t -= dt;
        if *t <= 0.0 {
            g.admin.pending_reset = None;
        }
    }
    // (host) players kicked are dropped once their notice is out
    let gone: Vec<usize> = std::mem::take(&mut g.admin.kicked_slots);
    for s in gone {
        g.admin.drop_slots.push(s);
    }
    // (host) the Steam IDs as it sees them, now and then
    if g.kind == Kind::Host {
        g.admin.ids_t -= dt;
        if g.admin.ids_t <= 0.0 {
            g.admin.ids_t = 3.0;
            let e = host_ids(g);
            g.out_events.push(Ev::Hub(e));
        }
    }
}

// ------------------------------------------------------------------ straight to a Steam user

/// Messages that change a player's saved things: sent straight from the issuer to the player
/// over Steam (the sender is proven by Steam, never by the host).
#[derive(Clone, Debug, PartialEq)]
pub enum Direct {
    Grant { item: ItemKey, n: u32 },
    Reset,
    Roster { id: u64, on: bool },
    /// An admin's gift made from nothing: proof that gift id `gid` is really theirs.
    GiftSeal { gid: u32 },
}

const DMAGIC: &[u8; 4] = b"DRAD";

impl Direct {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = DMAGIC.to_vec();
        match self {
            Direct::Grant { item, n } => {
                b.push(1);
                b.extend_from_slice(&item.to_bytes());
                b.extend_from_slice(&(*n as u16).to_le_bytes());
            }
            Direct::Reset => b.push(2),
            Direct::Roster { id, on } => {
                b.push(3);
                b.extend_from_slice(&id.to_le_bytes());
                b.push(*on as u8);
            }
            Direct::GiftSeal { gid } => {
                b.push(4);
                b.extend_from_slice(&gid.to_le_bytes());
            }
        }
        b
    }
    pub fn from_bytes(b: &[u8]) -> Option<Direct> {
        if b.len() < 5 || &b[..4] != DMAGIC {
            return None;
        }
        let r = &b[5..];
        Some(match b[4] {
            1 if r.len() >= 4 => Direct::Grant { item: ItemKey::from_bytes([r[0], r[1]])?, n: u16::from_le_bytes([r[2], r[3]]).clamp(1, 99) as u32 },
            2 => Direct::Reset,
            3 if r.len() >= 9 => Direct::Roster { id: u64::from_le_bytes(r[..8].try_into().ok()?), on: r[8] == 1 },
            4 if r.len() >= 4 => Direct::GiftSeal { gid: u32::from_le_bytes(r[..4].try_into().ok()?) },
            _ => return None,
        })
    }
}

/// A message straight from Steam user `from` (Steam vouches for who sent it).
pub fn on_direct(g: &mut Game, from: u64, bytes: &[u8]) {
    let Some(d) = Direct::from_bytes(bytes) else { return };
    let role = g.role_of_id(from);
    let who = g.slot_of_id(from).map(|s| g.f[s].name.clone()).unwrap_or_else(|| format!("Steam user {from}"));
    let need = if matches!(d, Direct::Roster { .. }) { Role::Owner } else { Role::Admin };
    if role < need {
        g.admin.log_line(&who, from, &format!("{d:?}"), "refused: the sender has no such role here");
        return;
    }
    g.admin.log_line(&who, from, &format!("{d:?}"), "applied");
    match d {
        Direct::Grant { item, n } => {
            if let Some(h) = g.hub.as_mut() {
                h.bank.grant(item, n, &format!("from {who}"));
                h.bank.apply_to(&mut h.earned);
                h.toast(format!("{who} gave you {n} {}", item.name()));
            }
        }
        Direct::Reset => {
            g.admin.reset_req = true;
            g.admin.replies.push(format!("{who} reset your progress."));
        }
        Direct::Roster { id, on } => {
            g.admin.roster.set_admin(id, on);
            g.admin.roster.save();
        }
        Direct::GiftSeal { gid } => {
            if let Some(h) = g.hub.as_mut() {
                h.gifts.seals.push((gid, from));
            }
        }
    }
}
