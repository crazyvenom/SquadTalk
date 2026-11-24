//! End-to-end test of the server: joining, passwords, owner code, rooms,
//! chat and voice relaying. Runs on GitHub with every build.

use squadtalk::protocol::*;
use squadtalk::server::{self, Options, Saved};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct TestClient {
    w: TcpStream,
    r: BufReader<TcpStream>,
    session: u32,
    token: u32,
    owner: bool,
    udp: UdpSocket,
}

fn send(w: &mut TcpStream, m: &C2S) {
    w.write_all(to_line(m).as_bytes()).unwrap();
}

fn read(r: &mut BufReader<TcpStream>) -> S2C {
    let mut line = String::new();
    r.read_line(&mut line).unwrap();
    serde_json::from_str(line.trim()).unwrap_or_else(|e| panic!("bad line {line:?}: {e}"))
}

fn try_join(port: u16, name: &str, secret: &str, password: &str) -> Result<TestClient, String> {
    let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut w = s.try_clone().unwrap();
    let mut r = BufReader::new(s);
    send(
        &mut w,
        &C2S::Hello { version: VERSION, name: name.into(), secret: secret.into(), password: password.into() },
    );
    match read(&mut r) {
        S2C::Welcome { session, udp_token, owner } => {
            let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
            udp.connect(("127.0.0.1", port)).unwrap();
            udp.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            Ok(TestClient { w, r, session, token: udp_token, owner, udp })
        }
        S2C::Denied { reason } => Err(reason),
        other => panic!("unexpected {other:?}"),
    }
}

fn wait_state(c: &mut TestClient, pred: impl Fn(&[RoomInfo], &[UserInfo]) -> bool) -> (Vec<RoomInfo>, Vec<UserInfo>) {
    for _ in 0..20 {
        if let S2C::State { rooms, users } = read(&mut c.r) {
            if pred(&rooms, &users) {
                return (rooms, users);
            }
        }
    }
    panic!("expected state never arrived");
}

fn udp_ping(c: &TestClient) {
    let mut p = vec![PKT_PING];
    p.extend_from_slice(&c.session.to_le_bytes());
    p.extend_from_slice(&c.token.to_le_bytes());
    p.extend_from_slice(&42u64.to_le_bytes());
    c.udp.send(&p).unwrap();
    let mut buf = [0u8; 64];
    let n = c.udp.recv(&mut buf).unwrap();
    assert_eq!(n, 9);
    assert_eq!(buf[0], PKT_PING);
    assert_eq!(u64::from_le_bytes(buf[1..9].try_into().unwrap()), 42);
}

#[test]
fn full_flow() {
    let port = 39987;
    let saved: Arc<Mutex<Option<(Vec<String>, Vec<String>)>>> = Arc::new(Mutex::new(None));
    let saved2 = saved.clone();
    let _server = server::start(Options {
        port,
        password: "letmein".into(),
        owner_code: "OWNER-123".into(),
        owner_uids: vec![],
        rooms: vec!["Lobby".into(), "Squad A".into()],
        on_save: Arc::new(move |s: Saved| *saved2.lock().unwrap() = Some((s.rooms, s.owner_uids))),
        log: false,
    })
    .expect("server starts");

    // Wrong password is refused.
    let err = try_join(port, "Mallory", "mallory-secret-0001", "nope").err().expect("denied");
    assert!(err.contains("password"), "{err}");

    // Owner code makes you owner and is remembered.
    let owner_secret = "alice-secret-00000001";
    let mut alice = try_join(port, "Alice", owner_secret, "OWNER-123").unwrap();
    assert!(alice.owner);
    let remembered = saved.lock().unwrap().clone().expect("owner saved");
    assert_eq!(remembered.1, vec![uid_of(owner_secret)]);

    // Normal password works, not an owner.
    let mut bob = try_join(port, "Bob", "bob-secret-0000000001", "letmein").unwrap();
    assert!(!bob.owner);
    wait_state(&mut alice, |_, u| u.len() == 2);
    wait_state(&mut bob, |_, u| u.len() == 2);

    // Register both voice connections.
    udp_ping(&alice);
    udp_ping(&bob);

    // Alice talks, Bob hears it, tagged with Alice's session.
    let mut p = vec![PKT_VOICE];
    p.extend_from_slice(&alice.session.to_le_bytes());
    p.extend_from_slice(&alice.token.to_le_bytes());
    p.extend_from_slice(&7u16.to_le_bytes());
    p.extend_from_slice(&[9, 8, 7, 6, 5]);
    alice.udp.send(&p).unwrap();
    let mut buf = [0u8; 64];
    let n = bob.udp.recv(&mut buf).unwrap();
    assert_eq!(buf[0], PKT_VOICE);
    assert_eq!(u32::from_le_bytes(buf[1..5].try_into().unwrap()), alice.session);
    assert_eq!(u16::from_le_bytes(buf[5..7].try_into().unwrap()), 7);
    assert_eq!(&buf[7..n], &[9, 8, 7, 6, 5]);

    // A forged token is ignored.
    let mut bad = p.clone();
    bad[5] ^= 0xFF;
    alice.udp.send(&bad).unwrap();
    bob.udp.set_read_timeout(Some(Duration::from_millis(400))).unwrap();
    assert!(bob.udp.recv(&mut buf).is_err(), "forged packet must not be relayed");

    // Bob can't use owner tools.
    send(&mut bob.w, &C2S::AddRoom { name: "Hacked".into() });
    send(&mut bob.w, &C2S::Ping);
    assert!(matches!(read(&mut bob.r), S2C::Pong));

    // Alice adds a room and moves Bob there.
    send(&mut alice.w, &C2S::AddRoom { name: "Squad B".into() });
    let (rooms, _) = wait_state(&mut alice, |r, _| r.len() == 3);
    assert!(!rooms.iter().any(|r| r.name == "Hacked"));
    let squad_b = rooms.iter().find(|r| r.name == "Squad B").unwrap().id;
    send(&mut alice.w, &C2S::Move { session: bob.session, room: squad_b });
    wait_state(&mut bob, |_, u| u.iter().any(|x| x.name == "Bob" && x.room == squad_b));

    // Different rooms: voice is not relayed.
    alice.udp.send(&p).unwrap();
    assert!(bob.udp.recv(&mut buf).is_err(), "voice must stay inside the room");

    // Chat reaches people in the same room.
    send(&mut bob.w, &C2S::Join { room: rooms[0].id });
    wait_state(&mut alice, |_, u| u.iter().all(|x| x.room == rooms[0].id));
    send(&mut bob.w, &C2S::Chat { text: "hello".into() });
    loop {
        if let S2C::Chat { name, text, .. } = read(&mut alice.r) {
            assert_eq!((name.as_str(), text.as_str()), ("Bob", "hello"));
            break;
        }
    }

    // Kick.
    send(&mut alice.w, &C2S::Kick { session: bob.session });
    loop {
        match read(&mut bob.r) {
            S2C::Kicked { .. } => break,
            _ => continue,
        }
    }
    wait_state(&mut alice, |_, u| u.len() == 1);

    // Rooms were saved.
    let remembered = saved.lock().unwrap().clone().unwrap();
    assert_eq!(remembered.0, vec!["Lobby", "Squad A", "Squad B"]);
}
