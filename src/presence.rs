//! Rich presence: the line friends see beside your name in Steam ("In the Varrow Centre",
//! "Ranked 1v1 - Gold Writ - 3-2 on Sunfall Keep", "Raiding the Ashen Vault - room 4/5"), and
//! the Join button where a game can be joined.
//!
//! Two forms are sent: `status`, the whole line in plain text (shown under any App ID, including
//! the test one), and `steam_display` with its substitution keys, which Steam localizes from
//! steam/rich_presence.vdf once that file is uploaded for the game's own App ID. `connect` makes
//! the Join button (a ranked match is joined to watch), and `steam_player_group` lets Steam show
//! a party together.

/// Where the player is, as far as their friends need to know.
#[derive(Clone, Debug, PartialEq)]
pub enum Where {
    Menu,
    Hub,
    Ranked { rank: String, me: u32, them: u32, map: String },
    /// A casual match: mode name, map, and the score line (yours first) when it's two-sided.
    Match { mode: String, map: String, score: Option<(u32, u32)> },
    Gauntlet { wave: u32 },
    Raid { room: usize, rooms: usize, hard: bool },
    Training,
    Watching { map: String },
    Replays,
}

/// What to send: the plain line, the localization token and its values, the connect string.
#[derive(Clone, Debug, PartialEq)]
pub struct Presence {
    pub status: String,
    pub display: &'static str,
    pub subs: Vec<(&'static str, String)>,
    pub connect: Option<String>,
    pub group: Option<(String, usize)>,
}

/// The line and token for a place. `joinable`: the match lobby friends can join (and whether
/// only to watch); `party`: the party's lobby and size.
pub fn describe(w: &Where, joinable: Option<(u64, bool)>, party: Option<(u64, usize)>) -> Presence {
    let (status, display, subs): (String, &'static str, Vec<(&'static str, String)>) = match w {
        Where::Menu => ("In the menus".into(), "#Status_Menu", vec![]),
        Where::Hub => ("In the Varrow Centre".into(), "#Status_Hub", vec![]),
        Where::Ranked { rank, me, them, map } => (
            format!("Ranked 1v1 - {rank} - {me}-{them} on {map}"),
            "#Status_Ranked",
            vec![("rank", rank.clone()), ("score", format!("{me}-{them}")), ("map", map.clone())],
        ),
        Where::Match { mode, map, score } => match score {
            Some((a, b)) => (format!("{mode} - {a}-{b} on {map}"), "#Status_MatchScore", vec![("mode", mode.clone()), ("score", format!("{a}-{b}")), ("map", map.clone())]),
            None => (format!("{mode} on {map}"), "#Status_Match", vec![("mode", mode.clone()), ("map", map.clone())]),
        },
        Where::Gauntlet { wave } => (format!("The Gauntlet - wave {wave}"), "#Status_Gauntlet", vec![("wave", wave.to_string())]),
        Where::Raid { room, rooms, hard } => {
            let rm = format!("{}/{}", room + 1, rooms);
            if *hard {
                (format!("Raiding the Ashen Vault (Sealed Writ) - room {rm}"), "#Status_RaidHard", vec![("room", rm)])
            } else {
                (format!("Raiding the Ashen Vault - room {rm}"), "#Status_Raid", vec![("room", rm)])
            }
        }
        Where::Training => ("Training with the dummies".into(), "#Status_Training", vec![]),
        Where::Watching { map } => (format!("Watching a match on {map}"), "#Status_Watching", vec![("map", map.clone())]),
        Where::Replays => ("Watching replays".into(), "#Status_Replays", vec![]),
    };
    let connect = joinable.map(|(l, watch)| crate::party::lobby_connect(l, watch));
    let group = party.filter(|p| p.1 > 1).map(|(id, n)| (id.to_string(), n));
    Presence { status, display, subs, connect, group }
}

/// Send it (only changes reach Steam: crate::platform::steam_api::RealSteam keeps the last values).
pub fn send(api: &mut dyn crate::platform::steam_api::SteamApi, p: &Presence) {
    api.presence("status", Some(&p.status));
    if api.features().presence_tokens {
        for (k, v) in &p.subs {
            api.presence(k, Some(v));
        }
        api.presence("steam_display", Some(p.display));
    }
    api.presence("connect", p.connect.as_deref());
    match &p.group {
        Some((g, n)) => {
            api.presence("steam_player_group", Some(g));
            api.presence("steam_player_group_size", Some(&n.to_string()));
        }
        None => {
            api.presence("steam_player_group", None);
            api.presence("steam_player_group_size", None);
        }
    }
}

/// Every token and its English line, for steam/rich_presence.vdf (`%key%` are substitutions).
pub const TOKENS: [(&str, &str); 12] = [
    ("#Status_Menu", "In the menus"),
    ("#Status_Hub", "In the Varrow Centre"),
    ("#Status_Ranked", "Ranked 1v1 - %rank% - %score% on %map%"),
    ("#Status_Match", "%mode% on %map%"),
    ("#Status_MatchScore", "%mode% - %score% on %map%"),
    ("#Status_Gauntlet", "The Gauntlet - wave %wave%"),
    ("#Status_Raid", "Raiding the Ashen Vault - room %room%"),
    ("#Status_RaidHard", "Raiding the Ashen Vault (Sealed Writ) - room %room%"),
    ("#Status_Training", "Training with the dummies"),
    ("#Status_Watching", "Watching a match on %map%"),
    ("#Status_Replays", "Watching replays"),
    ("#Status_Party", "In a party of %size%"),
];

/// The localization file to upload on the partner site (Rich Presence > Localization).
pub fn vdf() -> String {
    let mut s = String::from("\"lang\"\n{\n\t\"Language\"\t\"english\"\n\t\"Tokens\"\n\t{\n");
    for (k, v) in TOKENS {
        s += &format!("\t\t\"{k}\"\t\"{v}\"\n");
    }
    s += "\t}\n}\n";
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::steam_api::{FakeSteam, SteamApi};

    #[test]
    fn friends_see_the_lines_from_the_brief_with_join_buttons() {
        assert_eq!(describe(&Where::Hub, Some((8, false)), None).status, "In the Varrow Centre");
        let r = describe(&Where::Ranked { rank: "Gold Writ".into(), me: 3, them: 2, map: "Sunfall Keep".into() }, Some((9, true)), None);
        assert_eq!(r.status, "Ranked 1v1 - Gold Writ - 3-2 on Sunfall Keep");
        assert_eq!(r.connect.as_deref(), Some("+connect_lobby 9 +watch"), "a ranked match is joined to watch");
        let raid = describe(&Where::Raid { room: 3, rooms: 5, hard: false }, None, Some((44, 3)));
        assert_eq!(raid.status, "Raiding the Ashen Vault - room 4/5");
        assert_eq!(raid.connect, None, "a raid in progress can't be joined");
        assert_eq!(raid.group, Some(("44".into(), 3)));
        // every token used is in the localization file, with the same substitutions
        let file = vdf();
        for w in [Where::Menu, Where::Hub, Where::Gauntlet { wave: 7 }, Where::Training, Where::Replays, Where::Watching { map: "x".into() }, Where::Match { mode: "Duel".into(), map: "m".into(), score: Some((1, 0)) }, Where::Match { mode: "FFA".into(), map: "m".into(), score: None }, Where::Raid { room: 0, rooms: 5, hard: true }] {
            let p = describe(&w, None, None);
            let line = TOKENS.iter().find(|t| t.0 == p.display).expect("token listed").1;
            assert!(file.contains(p.display));
            for (k, v) in &p.subs {
                assert!(line.contains(&format!("%{k}%")), "{k} in {line}");
                assert!(p.status.contains(v.as_str()));
            }
        }
    }

    #[test]
    fn under_the_test_app_only_the_plain_line_is_sent() {
        let p = describe(&Where::Ranked { rank: "Silver Writ".into(), me: 1, them: 0, map: "The Undercroft".into() }, Some((5, true)), Some((6, 2)));
        let mut real = FakeSteam::new(1);
        send(&mut real, &p);
        assert_eq!(real.rich.get("steam_display").map(String::as_str), Some("#Status_Ranked"));
        assert_eq!(real.rich.get("rank").map(String::as_str), Some("Silver Writ"));
        assert_eq!(real.rich.get("connect").map(String::as_str), Some("+connect_lobby 5 +watch"));
        assert_eq!(real.rich.get("steam_player_group_size").map(String::as_str), Some("2"));
        let mut test = FakeSteam::new(2).as_test_app();
        send(&mut test, &p);
        assert!(test.rich.get("steam_display").is_none(), "no localization file under 480");
        assert_eq!(test.rich.get("status").map(String::as_str), Some("Ranked 1v1 - Silver Writ - 1-0 on The Undercroft"));
        assert!(test.features().friends);
        // leaving the lobby takes the Join button away
        send(&mut real, &describe(&Where::Menu, None, None));
        assert!(real.rich.get("connect").is_none() && real.rich.get("steam_player_group").is_none());
    }
}
