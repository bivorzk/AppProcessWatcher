# AppWatch – Windows alkalmazásonkénti hálózati és HTTP forgalomfigyelő

## Asztali rendszerprogramozási és hálózatkezelési projekt

### Rust nyelven

A projekt célja egy Windows operációs rendszerre készülő asztali alkalmazás létrehozása, amely képes a futó alkalmazások hálózati kapcsolatainak megfigyelésére, valamint támogatott esetben az alkalmazások által küldött HTTP és HTTPS kérések megjelenítésére.

A kész alkalmazásban a felhasználó kiválaszthat például egy `Discord.exe`, `Steam.exe`, böngésző vagy játék folyamatot, majd megtekintheti annak hálózati aktivitását.

A programnak különbséget kell tennie:

- TCP kapcsolat,
- UDP kapcsolat,
- HTTP kérés,
- HTTPS-en keresztül dekódolt HTTP kérés,
- nem dekódolható TLS kapcsolat,
- QUIC forgalom,
- ismeretlen vagy egyedi protokoll

között.

A projekt megoldásának megkezdéséhez hozzon létre egy Rust workspace-t `AppWatch` néven!

A projekt ajánlott könyvtár- és fájlszerkezete:

```text
AppWatch/
├── Cargo.toml
└── crates/
    ├── apppw-core/
    ├── apppw-windows/
    ├── apppw-capture/
    ├── apppw-proxy/
    ├── apppw-storage/
    └── apppw-ui/
```

Az `AppWatch/` és a `crates/` könyvtárak, a gyökérben lévő `Cargo.toml` pedig fájl. Minden `apppw-*` elem egy külön crate könyvtára, saját `Cargo.toml` fájllal és `src/` könyvtárral.

A gyökérben lévő `Cargo.toml` workspace-manifest legyen, ne külön futtatható Rust package. Minimális tartalma:

```toml
[workspace]
resolver = "3"
members = ["crates/*"]
```

Az `apppw-core`, `apppw-windows`, `apppw-capture`, `apppw-proxy` és `apppw-storage` library crate legyen `src/lib.rs` belépési fájllal. Az `apppw-ui` binary crate legyen `src/main.rs` belépési fájllal.

## Hogyan kell értelmezni a feladatokat?

Ez a rövid útmutató főleg akkor hasznos, ha Java után most kezd Rusttal foglalkozni.

- A `crate` egy külön fordítható Rust-projekt, nagyjából egy Maven- vagy Gradle-modulhoz hasonló.
- Nagyobb megoldásnál a kód külön modulfájlokba is kerülhet, például `src/process.rs` fájlba. Ezt a modult a `lib.rs` fájlban deklarálni és szükség esetén publikusan exportálni kell.
- A Rust `struct` nagyjából egy csak adatokat tároló Java-osztálynak felel meg. A metódusai külön `impl TípusNév` blokkba kerülnek.
- Az `enum` Rustban több adatot is hordozhat. Emiatt a Java `enum` és egy egyszerű sealed class használatához is hasonlíthat.
- A `trait` egy Java interface-hez hasonló szerződés. A `#[derive(...)]` bizonyos trait-ek implementációját automatikusan elkészíti.
- Az `Option<T>` azt jelenti, hogy az érték lehet `Some(value)` vagy hiányozhat, ekkor `None`.
- A `Result<T, E>` siker esetén `Ok(value)`, hiba esetén `Err(error)` értéket tartalmaz. A dokumentumban szereplő rövid `Result<T>` alak használatához válasszon konkrét hibatípust vagy egy olyan aliast, mint az `anyhow::Result<T>`.
- A `Vec<T>` egy növelhető lista, nagyjából a Java `ArrayList<T>` megfelelője.
- A `String` birtokolt, módosítható szöveg. A `&str` egy kölcsönzött szövegrészlet.
- A `Self` az aktuálisan implementált típust jelenti.
- A `&self` egy már létező példány kölcsönzött elérését jelenti, hasonlóan ahhoz, amikor Java metódusban a `this` mezőit olvassa.
- A kódmintákban szereplő `...` csak kihagyott rész jelölése, nem másolható be Rust-kódba.

A feladatokban név szerint kért, más crate-ek által használt típusok és függvények legyenek publikusak (`pub`). Az adatmodellek felsorolt mezői ennél a tanulóprojektnél szintén lehetnek `pub` mezők; getterek készítése nem kötelező. Más crate típusának használatakor vegye fel azt path dependencyként a használó crate `Cargo.toml` fájljába, például:

```toml
[dependencies]
apppw-core = { path = "../apppw-core" }
```

A projekt során használható főbb technológiák:

```text
Rust
egui / eframe
Tokio
windows-rs
WinDivert
windivert vagy saját FFI wrapper
Hyper
rustls
rcgen
Serde
serde_json
SQLite / rusqlite
tracing
tracing-subscriber
anyhow (opcionális)
```

# Külső crate-ek – kezdő gyorstalpaló

Az alábbi példák egymástól független, kisméretű minták. Azt mutatják meg, hogyan kell elkezdeni az adott crate használatát; nem helyettesítik a teljes feladatmegoldást.

Fontos: a dependency mindig annak a crate-nek a `Cargo.toml` fájljába kerüljön, amely közvetlenül használja. Ne tegyen minden dependencyt automatikusan a workspace gyökér-manifestjébe.

## `serde` – Rust-értékek menthető formára alakítása

Kapcsolódó feladatok: 1., 10., 23.

A Serde a Rust `struct` és `enum` értékek szerializálását és visszaolvasását támogatja. Ez hasonló ahhoz, amikor Java nyelven a Jackson egy objektumot JSON-ná alakít.

Az `apppw-core/Cargo.toml` fájlban:

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
```

Használat:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Example {
    pub id: u64,
    pub name: String,
}
```

A `derive` feature teszi elérhetővé a `Serialize` és `Deserialize` automatikus implementációját. A Serde önmagában nem jelent JSON-t; a JSON-fájl előállításához a `serde_json` crate szükséges.

Dokumentáció: [Serde derive](https://serde.rs/derive.html).

## `windows` – Windows API-k meghívása

Kapcsolódó feladatok: 4. és 6.

A `windows` crate Rust bindingokat biztosít a Win32 API-khoz. Ez nem egy teljesen biztonságos, magas szintű process library: sok Win32 függvény `unsafe`, mert nyers pointereket és operációsrendszer-handle-öket használ.

Az `apppw-windows/Cargo.toml` fájlban kezdetben:

```toml
[dependencies]
apppw-core = { path = "../apppw-core" }
windows = { version = "0.62", features = [
    "Win32_Foundation",
    "Win32_System_Diagnostics_ToolHelp",
    "Win32_System_Threading",
    "Win32_NetworkManagement_IpHelper",
    "Win32_Networking_WinSock",
] }
```

A feature-ök csak a szükséges Windows API-modulokat kapcsolják be. Ha a fordító szerint egy import nem található, először az adott API dokumentációjában ellenőrizze a szükséges feature nevét.

### Folyamatok felsorolásának alapja

A 4. feladat megoldásának fő lépései:

1. `CreateToolhelp32Snapshot` segítségével készítsen pillanatképet a folyamatokról.
2. Állítsa be a `PROCESSENTRY32W.dwSize` mezőt.
3. A `Process32FirstW`, majd ismételten a `Process32NextW` segítségével járja be a folyamatokat.
4. A `th32ProcessID` mező a PID, az `szExeFile` pedig UTF-16 formában tartalmazza a nevet.
5. A snapshot handle-t mindig zárja le `CloseHandle` hívással.

Egyszerűsített minta:

```rust
use std::mem::size_of;
use windows::core::Result;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW,
    PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

fn list_process_names() -> Result<Vec<(u32, String)>> {
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)? };

    let result = (|| {
        let mut entry = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut processes = Vec::new();

        unsafe { Process32FirstW(snapshot, &mut entry)? };

        loop {
            let name_length = entry
                .szExeFile
                .iter()
                .position(|character| *character == 0)
                .unwrap_or(entry.szExeFile.len());

            let name = String::from_utf16_lossy(&entry.szExeFile[..name_length]);
            processes.push((entry.th32ProcessID, name));

            if unsafe { Process32NextW(snapshot, &mut entry) }.is_err() {
                break;
            }
        }

        Ok(processes)
    })();

    unsafe { CloseHandle(snapshot)? };
    result
}
```

Ez még csak PID-et és nevet kérdez le. A futtatható fájl útvonalához általában `OpenProcess` és `QueryFullProcessImageNameW` szükséges. Ezek hozzáférési hibát adhatnak például rendszerfolyamatoknál; ilyenkor az `executable_path` legyen `None`, és a feldolgozás folytatódjon.

A 6. feladatnál a `GetExtendedTcpTable` és `GetExtendedUdpTable` függvények használhatók. Ezeknél gyakori Win32-minta, hogy az első hívás csak a szükséges bufferméretet adja meg, ezután le kell foglalni a buffert, majd másodszor is meg kell hívni a függvényt. A pointert csak addig használja, amíg a mögötte lévő `Vec<u8>` él és nem kerül átméretezésre.

Dokumentáció: [ToolHelp API](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/System/Diagnostics/ToolHelp/index.html) és [IP Helper API](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/NetworkManagement/IpHelper/index.html).

## `windivert` – hálózati csomagok figyelése

Kapcsolódó feladatok: 5–9. és B6.

A `windivert` crate a natív WinDivert driver Rust wrappere. Csak Windows alatt működik, és a futtatáshoz általában rendszergazdai jogosultság, valamint a megfelelő WinDivert DLL és driver szükséges.

Az `apppw-capture/Cargo.toml` fájlban:

```toml
[dependencies]
apppw-core = { path = "../apppw-core" }
windivert = "0.6"
```

Minimális, passzív megfigyelési minta:

```rust
use windivert::prelude::{WinDivert, WinDivertFlags};

fn capture_one_packet() -> Result<(), Box<dyn std::error::Error>> {
    let flags = WinDivertFlags::new().set_sniff();
    let handle = WinDivert::network("(tcp or udp)", 0, flags)?;

    let packet = handle.recv(None)?;
    println!("Captured {} bytes", packet.data.len());

    Ok(())
}
```

A `sniff` flag esetén a driver lemásolja a csomagot a programnak, miközben az eredeti továbbhalad. Ez a legegyszerűbb választás egy csak megfigyelő alkalmazáshoz, mert nem kell minden csomagot visszainjektálni.

Ha a handle nem `sniff` módban nyílik meg, a csomagot a program eltéríti a hálózati stackből. Ekkor a változatlan csomagot `handle.send(&packet)?` hívással vissza kell küldeni, különben a hálózati kapcsolat megszakadhat.

A `recv` blokkoló hívás, ezért külön worker threaden fusson. A csomag bájtjai a `packet.data`, az irányra és interfészre vonatkozó metaadatok pedig a `packet.address` mezőben találhatók. A TCP/IP headerek feldolgozásához használható a wrapper által már alkalmazott packet parser, vagy egy külön parser, de ugyanazt a packetet ne parse-olja több, egymástól független módon.

Dokumentáció: [`windivert::WinDivert`](https://docs.rs/windivert/latest/windivert/struct.WinDivert.html).

## `tokio` – aszinkron feladatok és csatornák

Kapcsolódó feladatok: 5., 12–14., 17. és 24.

A Tokio egy async runtime. A Java `CompletableFuture`, executor és blocking queue fogalmaihoz hasonló feladatokat kezel, de Rust `async fn`, `.await` és taskok formájában.

Azokban a crate-ekben, amelyek async hálózati kódot futtatnak:

```toml
[dependencies]
tokio = { version = "1", features = ["macros", "rt-multi-thread", "net", "sync", "time"] }
```

Minimális mpsc channel:

```rust
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (sender, mut receiver) = mpsc::channel::<String>(256);

    tokio::spawn(async move {
        let _ = sender.send(String::from("packet captured")).await;
    });

    while let Some(event) = receiver.recv().await {
        println!("{event}");
    }
}
```

- A `Sender` klónozható, ezért több producer küldhet eseményeket.
- Egy `mpsc::Receiver` csak egy consumerhez tartozik.
- A `256` a puffer kapacitása. A véges puffer megakadályozza a memória korlátlan növekedését.
- A `tokio::spawn(async move { ... })` átadja a blokk által használt értékek tulajdonjogát az új tasknak.
- Blokkoló Win32- vagy WinDivert-hívást ne futtasson közvetlenül async taskban. Használjon külön OS threadet vagy indokolt esetben `tokio::task::spawn_blocking` hívást.

Dokumentáció: [Tokio taskok](https://tokio.rs/tokio/tutorial/spawning) és [Tokio channelek](https://tokio.rs/tokio/tutorial/channels).

## `hyper`, `hyper-util` és `http-body-util` – HTTP feldolgozás

Kapcsolódó feladat: 12.

- A `hyper` tartalmazza a HTTP request/response típusokat és a HTTP/1, illetve HTTP/2 kapcsolatkezelést.
- A `hyper-util` összeköti a Hypert például a Tokio I/O típusaival.
- A `http-body-util` segít HTTP body-k létrehozásában, streamelésében vagy összegyűjtésében.

Az `apppw-proxy/Cargo.toml` fájlban:

```toml
[dependencies]
apppw-core = { path = "../apppw-core" }
tokio = { version = "1", features = ["macros", "rt-multi-thread", "net"] }
hyper = { version = "1", features = ["full"] }
hyper-util = { version = "0.1", features = ["full"] }
http-body-util = "0.1"
```

Egy beérkező HTTP-kérés alapadatainak kiolvasása:

```rust
use std::convert::Infallible;
use http_body_util::Full;
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};

async fn inspect_request(
    request: Request<Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    println!("Method: {}", request.method());
    println!("URI: {}", request.uri());

    for (name, value) in request.headers() {
        println!("{name}: {value:?}");
    }

    Ok(Response::new(Full::new(Bytes::from("proxy placeholder"))))
}
```

A `service_fn(inspect_request)` ezt az async függvényt Hyper service-szé alakítja. A `TcpListener` által elfogadott streamet `hyper_util::rt::TokioIo::new(stream)` adapterbe kell csomagolni, majd például `hyper::server::conn::http1::Builder::serve_connection` kezeli a kapcsolatot.

A fenti kód még nem proxy: csak helyi választ küld. Valódi proxynál a kérést tovább kell küldeni a célkiszolgálónak, majd annak válaszát kell visszaadni. Ha a body-t `BodyExt::collect()` segítségével teljesen memóriába tölti, állítson be méretkorlátot; nagy vagy végtelen streamet nem szabad korlátlanul gyűjteni.

Dokumentáció: [Hyper server](https://hyper.rs/guides/1/server/hello-world/) és [Hyper client](https://hyper.rs/guides/1/client/basic/).

## `rustls`, `tokio-rustls` és `rcgen` – TLS

Kapcsolódó feladat: 13.

- A `rustls` kezeli a TLS protokollt és a tanúsítvány-ellenőrzést.
- A `tokio-rustls` a rustls TLS streamjeit a Tokio async I/O rendszeréhez kapcsolja.
- Az `rcgen` tanúsítványt és privát kulcsot állít elő.

Az `apppw-proxy/Cargo.toml` fájlban:

```toml
[dependencies]
rustls = "0.23"
tokio-rustls = "0.26"
rcgen = "0.14"
```

Egyszerű, egyetlen hostnévre érvényes szerverkonfiguráció:

```rust
use std::sync::Arc;
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::ServerConfig;
use tokio_rustls::TlsAcceptor;

fn make_acceptor(host: &str) -> Result<TlsAcceptor, Box<dyn std::error::Error>> {
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec![host.to_owned()])?;

    let certificate_chain = vec![cert.der().clone()];
    let private_key = PrivateKeyDer::Pkcs8(
        PrivatePkcs8KeyDer::from(signing_key.serialize_der()),
    );

    let config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certificate_chain, private_key)?;

    Ok(TlsAcceptor::from(Arc::new(config)))
}
```

Egy elfogadott `tokio::net::TcpStream` TLS streammé alakítása:

```rust
let tls_stream = acceptor.accept(tcp_stream).await?;
```

Ez csak a TLS-alapokat mutatja. HTTPS inspection proxyhoz szükséges egy helyi CA, hostonként aláírt tanúsítvány, upstream TLS klienskapcsolat és CONNECT-kezelés is. A kliensnek előzetesen meg kell bíznia a helyi CA-ban. Ne kapcsolja ki az upstream szerver tanúsítványának ellenőrzését, és ne naplózza a CA privát kulcsát.

Dokumentáció: [rustls konfiguráció](https://docs.rs/rustls/latest/rustls/struct.ConfigBuilder.html), [`tokio-rustls`](https://docs.rs/tokio-rustls/latest/tokio_rustls/) és [`rcgen`](https://docs.rs/rcgen/latest/rcgen/).

## `rusqlite` – SQLite adatbázis

Kapcsolódó feladatok: 15., 22. és 23.

A `rusqlite::Connection` egy adatbázis-kapcsolatot jelent. A Java JDBC `Connection` és `PreparedStatement` fogalmaihoz hasonló, de a paramétereket gyakran a `params!` makróval adja át.

Az `apppw-storage/Cargo.toml` fájlban:

```toml
[dependencies]
apppw-core = { path = "../apppw-core" }
rusqlite = { version = "0.40", features = ["bundled"] }
```

A `bundled` feature saját SQLite-verziót fordít a programhoz, ezért Windows alatt kevesebb külön telepítési és linkelési hibát okoz.

Minimális példa:

```rust
use rusqlite::{params, Connection, Result};

fn save_example() -> Result<()> {
    let connection = Connection::open("appwatch.db")?;

    connection.execute(
        "CREATE TABLE IF NOT EXISTS processes (
            id INTEGER PRIMARY KEY,
            pid INTEGER NOT NULL,
            name TEXT NOT NULL
        )",
        (),
    )?;

    connection.execute(
        "INSERT INTO processes (pid, name) VALUES (?1, ?2)",
        params![14280_u32, "Discord.exe"],
    )?;

    Ok(())
}
```

A `?1` és `?2` SQL-paraméter. Ne építsen SQL-utasítást `format!` segítségével felhasználói adatokból; használjon paramétereket. A `rusqlite::Connection` nem `Sync`, ezért ne ossza meg közvetlenül több task között. Ennél a projektnél a legegyszerűbb egyetlen adatbázis-worker, amely channelen kapja a mentendő eseményeket.

Dokumentáció: [`rusqlite`](https://github.com/rusqlite/rusqlite).

## `eframe` és `egui` – grafikus felület

Kapcsolódó feladatok: 16–22. és B2–B5.

Az `egui` immediate-mode GUI: a felület minden frame-ben újra leírja, mit kell megjeleníteni. Nem hoz létre tartós JavaFX/Swing komponensobjektum-fát. A tartós adatokat, például a kiválasztott PID-et, saját `struct` mezőkben kell tárolni.

Az `apppw-ui/Cargo.toml` fájlban:

```toml
[dependencies]
apppw-core = { path = "../apppw-core" }
eframe = "0.36"
```

Minimális alkalmazás:

```rust
use eframe::egui;

#[derive(Default)]
struct AppWatchUi {
    selected_pid: Option<u32>,
}

impl eframe::App for AppWatchUi {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.heading("Processes");

        if ui.button("All processes").clicked() {
            self.selected_pid = None;
        }
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        "AppWatch",
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::new(AppWatchUi::default()))),
    )
}
```

A `.clicked()` csak abban a frame-ben `true`, amelyben a felhasználó megnyomta a gombot. A GUI metódusban ne végezzen blokkoló hálózati vagy adatbázis-műveletet. A háttérből érkező eseményeket rövid, nem blokkoló lépésben másolja át a UI állapotába.

Figyelem: régebbi eframe-verziók példái `update` metódust mutathatnak. A dokumentumban használt aktuális eframe 0.36 `App` trait-je `ui` metódust kér.

Dokumentáció: [`eframe::App`](https://docs.rs/eframe/latest/eframe/trait.App.html) és [`run_native`](https://docs.rs/eframe/latest/eframe/fn.run_native.html).

## `tracing` és `tracing-subscriber` – naplózás

Kapcsolódó feladat: a teljes alkalmazás hibakeresése és a 24. feladat.

A library crate-ek a `tracing` makróival eseményeket írnak. A futtatható `apppw-ui` egyszer, induláskor telepíti a `tracing-subscriber` kimenetet.

Az érintett library crate-ekben:

```toml
[dependencies]
tracing = "0.1"
```

Az `apppw-ui/Cargo.toml` fájlban ezen felül:

```toml
tracing-subscriber = "0.3"
```

Használat:

```rust
fn main() {
    tracing_subscriber::fmt().init();

    let pid = 14280_u32;
    tracing::info!(pid, "process selected");
    tracing::warn!(pid, "executable path unavailable");
}
```

A `println!` helyett a strukturált mezők — például `pid`, `connection_id` és `remote_ip` — később könnyebben szűrhetők. HTTP body-t, Authorization headert, Cookie-t, tanúsítványkulcsot vagy más titkot ne naplózzon.

Dokumentáció: [`tracing`](https://docs.rs/tracing/latest/tracing/) és [`tracing-subscriber`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/fmt/).

## `serde_json` – JSON export

Kapcsolódó feladat: 23.

Az exportot végző crate `Cargo.toml` fájljában:

```toml
[dependencies]
serde_json = "1"
```

Ha az exportált típus implementálja a `Serialize` trait-et:

```rust
use std::fs::File;

fn export_json<T: serde::Serialize>(
    path: &std::path::Path,
    value: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(path)?;
    serde_json::to_writer_pretty(file, value)?;
    Ok(())
}
```

A maszkolást az export előtt végezze el. A `Serialize` önmagában nem távolít el érzékeny mezőket.

Dokumentáció: [`serde_json`](https://docs.rs/serde_json/latest/serde_json/).

## `anyhow` – egyszerű alkalmazásszintű hibakezelés

Kapcsolódó feladatok: minden olyan hely, ahol több különböző crate hibáját kell egy függvényből továbbadni.

Az `anyhow` nem kötelező, de tanulóprojektben egyszerűbbé teszi a különböző hibák továbbadását:

```toml
[dependencies]
anyhow = "1"
```

```rust
use anyhow::{Context, Result};

fn load_config() -> Result<String> {
    std::fs::read_to_string("appwatch.toml")
        .context("az appwatch.toml konfiguráció nem olvasható")
}
```

A `?` továbbadja a hibát, a `context` pedig leírja, melyik magasabb szintű művelet sikertelen. Az `anyhow` főleg alkalmazáskódban praktikus; egy újrahasznosítható library publikus API-jánál később érdemes lehet saját hibatípust használni.

Dokumentáció: [`anyhow`](https://docs.rs/anyhow/latest/anyhow/).

---

# 1.) feladat [2 pont]

Az `apppw-core/src/lib.rs` fájlban hozzon létre egy új, publikus `enum` típust `NetworkProtocol` néven!

Ez azt jelenti, hogy a deklaráció `pub enum NetworkProtocol` formában kezdődjön.

Az enumeráció az alábbi változatokat tartalmazza:

```rust
TCP
UDP
HTTP
HTTPS
TLS
QUIC
UNKNOWN
```

Ugyanebben a crate-ben hozzon létre egy másik `enum` típust `ConnectionDirection` néven!

Az enumeráció az alábbi változatokat tartalmazza:

```rust
Inbound
Outbound
```

Mindkét enumeráció valósítsa meg vagy származtassa automatikusan az alábbi trait-eket:

```rust
Debug
Clone
Copy
PartialEq
Eq
Serialize
Deserialize
```

A `Serialize` és `Deserialize` derive használatához adja hozzá a `serde` crate-et az `apppw-core/Cargo.toml` fájlhoz a `derive` feature-rel, majd importálja a két trait-et a Rust-forrásban.

---

# 2.) feladat [2 pont]

Az `apppw-core/src/lib.rs` fájlban hozzon létre egy `ProcessInfo` nevű Rust `struct` típust.

Java nyelven ezt valószínűleg egy egyszerű adatokat tároló osztályként készítené el. Rustban ehhez egy `struct`, valamint a hozzá tartozó metódusokhoz egy `impl ProcessInfo` blokk szükséges.

A típus legyen publikus, vagyis a deklarációja `pub struct ProcessInfo` formában kezdődjön. Így más crate-ek, például az `apppw-windows`, is használhatják.

A `ProcessInfo` az alábbi három mezőt tartalmazza:

```rust
pid: u32
name: String
executable_path: Option<PathBuf>
```

- `pid`: a folyamat számszerű azonosítója;
- `name`: a folyamat neve, például `Discord.exe`;
- `executable_path`: a futtatható fájl elérési útja. Az `Option<PathBuf>` azt jelenti, hogy az útvonal vagy ismert (`Some(path)`), vagy nem ismert (`None`).

## Konstruktor

Rustban nincs a Javából ismert, osztály nevével megegyező konstruktor. Helyette készítsen egy publikus, társított `new` függvényt az `impl ProcessInfo` blokkon belül.

A függvény aláírása legyen:

```rust
pub fn new(
    pid: u32,
    name: String,
    executable_path: Option<PathBuf>,
) -> Self
```

A `new` hozzon létre és adjon vissza egy `ProcessInfo` értéket. Mindhárom paramétert mentse a vele azonos nevű mezőbe.

Példa a konstruktor használatára:

```rust
let process = ProcessInfo::new(
    14280,
    String::from("Discord.exe"),
    None,
);
```

## Szöveges megjelenítés (`Display`)

A Java `toString()` metódusához hasonló működéshez valósítsa meg a `std::fmt::Display` trait-et a `ProcessInfo` típushoz.

A `Display` határozza meg, mi jelenjen meg, amikor a `ProcessInfo` értékét `{}` használatával írja ki:

```rust
println!("{}", process);
```

A fenti példának pontosan ezt kell kiírnia:

```text
Discord.exe [PID: 14280]
```

Ehhez az implementációban a folyamat `name` és `pid` mezőjét kell a következő minta szerint formázni:

```text
<name> [PID: <pid>]
```

Nem az `executable_path` értékét kell megjeleníteni.

---

# 3.) feladat [3 pont]

Az `apppw-core` crate forráskódjában hozzon létre egy publikus `NetworkConnection` `struct` típust!

A felsorolt mezők legyenek `pub` mezők, mert más crate-eknek is olvasniuk kell őket:

```rust
id: u64
process: Option<ProcessInfo>
protocol: NetworkProtocol
direction: ConnectionDirection

local_ip: IpAddr
local_port: u16

remote_ip: IpAddr
remote_port: u16

hostname: Option<String>

bytes_sent: u64
bytes_received: u64
```

Hozzon létre az implementációban egy:

```rust
pub fn remote_address(&self) -> String
```

metódust!

Az eredmény formátuma például:

```text
162.159.135.232:443
```

Hozzon létre egy:

```rust
pub fn display_destination(&self) -> String
```

metódust is!

Amennyiben a `hostname` ismert, az eredmény például:

```text
discord.com:443
```

legyen!

Ha nincs hostname, akkor az IP-címet használja!

A cím összeállításánál az IPv6 formátumára is ügyeljen. Például az `::1` cím és a `443` port helyes alakja `[::1]:443`. Ehhez használható a szabványos `SocketAddr` típus; nem szükséges kézzel összefűzni az IP-címet és a portot.

---

# 4.) feladat [3 pont]

Az `apppw-windows` crate forráskódjában hozzon létre egy publikus `ProcessCollector` `struct` típust!

Hozzon létre benne egy:

```rust
pub fn list_processes(&self) -> Result<Vec<ProcessInfo>>
```

A `Result` itt nem egy önálló konkrét típus: választani kell hozzá hibát. Használható például `windows::core::Result<Vec<ProcessInfo>>`, vagy `anyhow::Result<Vec<ProcessInfo>>`, ha az `anyhow` dependency bekerül a crate-be. Egy megoldáson belül következetesen ugyanazt használja.

A `Vec<ProcessInfo>` a Java `List<ProcessInfo>` megfelelője. Siker esetén a metódus `Ok(processes)`, teljes lekérdezési hiba esetén `Err(error)` értéket adjon vissza.

metódust!

A metódus kérdezze le a Windows operációs rendszer aktuálisan futó folyamatait!

Minden olyan folyamathoz, amelyről rendelkezésre áll információ, mentse el:

- PID,
- folyamat nevét,
- futtatható állomány útvonalát.

A Windows API-k eléréséhez használhatja a `windows` crate-et.

Egyetlen sikertelen process-lekérdezés miatt a program ne szakítsa meg az összes folyamat feldolgozását! Az adott folyamat kihagyható, vagy a hiányzó útvonal `None` értékkel tárolható; a többi folyamat feldolgozása folytatódjon.

---

# 5.) feladat [4 pont]

Az `apppw-capture` crate forráskódjában hozzon létre egy publikus `PacketCapture` `struct` típust!

A struktúra használja a WinDivert könyvtárat a Windows hálózati forgalmának megfigyelésére.

Nyisson egy WinDivert handle-t a TCP- és UDP-forgalom megfigyelésére. Mivel a későbbi feladatok a küldött és fogadott bájtokat is számolják, a végleges szűrő mindkét irányt engedje át:

```text
(tcp or udp)
```

Az `outbound and (tcp or udp)` csak egy kezdeti, kizárólag kimenő forgalmat figyelő próbaszűrőként használható; azzal a fogadott forgalom nem számolható.

A program a beérkező csomagokból legalább az alábbi adatokat állapítsa meg:

```text
forrás IP
forrás port
cél IP
cél port
TCP vagy UDP
IPv4 vagy IPv6
forgalom iránya
csomag mérete
```

A csomag adataiból hozzon létre belső hálózati eseményt.

A program a megfigyelt csomagot alapesetben ne módosítsa!

Ha a WinDivert konfiguráció miatt a csomagot vissza kell injektálni a hálózati stackbe, azt változatlan formában tegye meg!

A capture loop egy ismétlődő feldolgozási ciklus, amely várja a következő csomagot. Ez külön worker threaden vagy aszinkron háttérfeladatban fusson, hogy a grafikus felület közben tovább tudjon frissülni.

---

# 6.) feladat [4 pont]

Az `apppw-windows` crate forráskódjában hozzon létre egy publikus `SocketProcessResolver` `struct` típust!

A struktúra feladata, hogy egy hálózati kapcsolatot lehetőség szerint Windows folyamathoz rendeljen.

A folyamat meghatározásához használhatja a Windows TCP és UDP kapcsolatlistáját.

TCP esetén az összerendelés során vizsgálja:

```text
local IP
local port
remote IP
remote port
protocol
```

A Windows TCP owner table tartalmaz helyi és távoli címet, portot, valamint PID-et. A Windows UDP owner table ezzel szemben csak a helyi címet, helyi portot és PID-et adja meg; távoli UDP-címet és portot nem. Emiatt UDP esetén csak a protokoll és a helyi végpont alapján próbáljon egyezést keresni. Ha ugyanahhoz a helyi UDP-porthoz több lehetséges folyamat tartozik, az eredmény legyen `None`.

Hivatalos referencia: [MIB_TCPROW_OWNER_PID](https://learn.microsoft.com/en-us/windows/win32/api/tcpmib/ns-tcpmib-mib_tcprow_owner_pid) és [MIB_UDPROW_OWNER_PID](https://learn.microsoft.com/en-us/windows/win32/api/udpmib/ns-udpmib-mib_udprow_owner_pid).

A sikeres összerendelés eredménye:

```text
PID → ProcessInfo
```

A resolver publikus metódusa adjon vissza `Option<ProcessInfo>` értéket: sikeres találatkor `Some(process_info)`, bizonytalan vagy sikertelen találatkor `None` legyen az eredmény. A pontos metódusnév szabadon választható.

Például:

```text
Discord.exe [PID: 14280]

192.168.1.10:53142
→
162.159.135.232:443
```

Ha a folyamat nem állapítható meg megbízhatóan, a program ne találjon ki PID-et!

Ilyen esetben:

```rust
process: None
```

értéket használjon!

---

# 7.) feladat [4 pont]

Az `apppw-core` crate forráskódjában hozzon létre egy publikus `NetworkEvent` `enum` típust!

Az enumeráció tartalmazza:

```rust
PacketCaptured(CapturedPacket),
ConnectionOpened(NetworkConnection),
ConnectionUpdated(NetworkConnection),
ConnectionClosed(u64)
```

A `...` nem érvényes Rust-szintaxis, ezért itt a konkrét `CapturedPacket` típust kell használni. A `CapturedPacket` részletes mezőit a 9. feladat adja meg; a 7. és 9. feladatot emiatt együtt is meg lehet valósítani.

Az `apppw-capture` crate forráskódjában hozzon létre egy publikus `ConnectionTracker` `struct` típust!

A struktúra a WinDivert által megfigyelt TCP és UDP csomagok alapján tartsa nyilván a hálózati kapcsolatokat.

Kapcsolat azonosításához használja az úgynevezett 5-tuple-t:

```text
protocol
source IP
source port
destination IP
destination port
```

A tracker tartsa nyilván legalább:

```text
kapcsolat kezdete
utolsó aktivitás
elküldött bájtok
fogadott bájtok
folyamat
```

Ezek lehetnek a `ConnectionTracker` saját belső rekordjának mezői; nem kötelező mindet utólag hozzáadni a 3. feladat `NetworkConnection` típusához.

---

# 8.) feladat [3 pont]

A `ConnectionTracker` legyen képes felismerni az új TCP kapcsolatokat.

Új kapcsolat esetén hozzon létre:

```rust
NetworkEvent::ConnectionOpened(connection)
```

eseményt.

Már ismert kapcsolat esetén frissítse:

```text
bytes_sent
bytes_received
last_seen
```

értékeket.

Megszűnt vagy hosszabb ideje inaktív kapcsolat esetén hozzon létre:

```rust
NetworkEvent::ConnectionClosed(connection_id)
```

eseményt.

UDP esetén a program kezelheti logikai sessionként az azonos 5-tuple-lel rendelkező csomagokat.

A két irány csomagjait ugyanahhoz a logikai kapcsolathoz kell rendelni. Vagyis az `A → B` és a válaszként érkező `B → A` tuple ne hozzon létre két külön kapcsolatot. A helyi végpont alapján döntse el, hogy a csomag bájtjai a `bytes_sent` vagy a `bytes_received` értéket növelik. Az inaktivitási időkorlát legyen egy névvel ellátott, könnyen módosítható konstans.

---

# 9.) feladat [4 pont]

Az `apppw-core` crate forráskódjában hozzon létre egy publikus `CapturedPacket` `struct` típust!

A struktúra tartalmazza legalább:

```rust
timestamp: SystemTime
protocol: NetworkProtocol
direction: ConnectionDirection

source_ip: IpAddr
source_port: Option<u16>

destination_ip: IpAddr
destination_port: Option<u16>

packet_size: usize
payload_size: usize
process: Option<ProcessInfo>
```

A WinDivert által átadott packetből hozzon létre egy `CapturedPacket` értéket. A `packet_size` a teljes csomag mérete, a `payload_size` pedig csak a protokoll-headerek utáni hasznos adat mérete; a kettő nem feltétlenül azonos.

A nyers packet tartalmának tárolása legyen opcionális, például egy `raw_packet: Option<Vec<u8>>` mezővel. Alapértelmezetten legyen `None`, hogy a program hosszabb futás során se használjon indokolatlanul sok memóriát.

---

# 10.) feladat [3 pont]

Az `apppw-core` crate forráskódjában hozzon létre egy publikus `HttpRequestInfo` `struct` típust!

Az adattagok:

```rust
id: u64
connection_id: Option<u64>

process: Option<ProcessInfo>

method: String
scheme: String
host: String
path: String

request_headers: Vec<HttpHeader>
request_body: Option<Vec<u8>>
request_body_size: usize

status_code: Option<u16>

response_headers: Vec<HttpHeader>
response_body: Option<Vec<u8>>
response_body_size: Option<usize>

started_at: SystemTime
duration_ms: Option<u128>
```

A body méretét akkor is töltse ki, ha magát a body tartalmát memóriahasználat vagy adatvédelmi okból nem tárolja. Az `Option` mezők itt azt jelzik, hogy az adat még nem érkezett meg, nem ismert, vagy szándékosan nincs eltárolva.

Ugyanebben a crate-ben hozzon létre egy `struct` adattípust `HttpHeader` néven, az alábbi mezőkkel:

```rust
name: String
value: String
```

Valósítson meg egy:

```rust
pub fn display_url(&self) -> String
```

metódust!

Például:

```text
https://discord.com/api/v10/users/@me
```

---

# 11.) feladat [3 pont]

Az `apppw-core` crate-ben írjon egy publikus:

```rust
pub fn redact_headers(headers: &[HttpHeader]) -> Vec<HttpHeader>
```

függvényt!

Az alábbi érzékeny headerek értékét:

```text
Authorization
Proxy-Authorization
Cookie
Set-Cookie
```

cserélje:

```text
[REDACTED]
```

értékre!

A headernevek összehasonlítása ne legyen kis- és nagybetűérzékeny! A bemeneti slice-ot ne módosítsa; készítsen új `Vec<HttpHeader>` eredményt, és abban cserélje le csak az érzékeny értékeket.

---

# 12.) feladat [5 pont]

Az `apppw-proxy` crate forráskódjában hozzon létre egy publikus `HttpProxy` `struct` típust!

A proxy a lokális gépen konfigurálható TCP porton figyeljen!

Alapértelmezett érték:

```text
127.0.0.1:8877
```

A proxy legyen képes HTTP kérések fogadására és továbbítására.

A kérésből mentse:

```text
HTTP method
host
path
headers
body size
```

A válaszból mentse:

```text
status code
headers
body size
response time
```

Minden kérésből hozzon létre egy `HttpRequestInfo` értéket! Rustban általában „értéknek” vagy „példánynak” nevezzük; ez felel meg a Java objektumpéldányának.

A kérés továbbítása azt jelenti, hogy a proxy kliensként kapcsolódik a célkiszolgálóhoz, elküldi neki a beérkezett kérést, majd a választ visszaküldi az eredeti kliensnek. A proxyhibát ne alakítsa sikeres HTTP-válasszá; jelenítsen meg vagy továbbítson értelmes hibát.

Ebben a feladatban a proxy még nem biztos, hogy ismeri a folyamatot. Ilyenkor a `HttpRequestInfo.process` értéke `None`; a folyamat hozzárendelése a 14. feladatban történik.

Használható:

```text
Tokio
Hyper
hyper-util
http-body-util
```

---

# 13.) feladat [5 pont]

Egészítse ki az `apppw-proxy` működését HTTPS támogatással!

TLS kommunikációhoz használhatja:

```text
rustls
tokio-rustls
```

Tanúsítványok létrehozásához használhatja:

```text
rcgen
```

A proxy csak olyan alkalmazások forgalmát dekódolja, amelyek megfelelően használják a konfigurált helyi proxyt és elfogadják annak tanúsítványát.

Ha egy kapcsolat nem dekódolható, akkor a WinDivert-alapú capture komponens továbbra is jelenítse meg például:

```text
Discord.exe

TLS
162.159.135.232:443

Contents unavailable
```

A program ne próbáljon certificate pinninget megkerülni!

Ez a funkció csak a felhasználó által kifejezetten a helyi proxy használatára beállított alkalmazásokhoz készüljön. A generált helyi hitelesítésszolgáltatói tanúsítvány privát kulcsát ne exportálja és ne naplózza. A nem támogatott TLS-kapcsolatot titkosított kapcsolatként kell tovább kezelni, nem hibás HTTP-kérésként.

---

# 14.) feladat [5 pont]

Az `apppw-core` crate-ben hozzon létre egy publikus `TrafficCorrelator` `struct` típust!

A komponens feladata három különböző adatforrás összekapcsolása:

```text
WinDivert packet
        +
Windows socket/process information
        +
HTTP proxy event
```

Az összerendeléshez vizsgálja:

```text
protocol
local IP
local port
remote IP
remote port
timestamp
PID
```

Sikeres összerendelés esetén például:

```text
Discord.exe [PID: 14280]

POST
https://discord.com/api/v10/channels/123/messages
```

jelenjen meg.

Az időbélyegeket egy kicsi, konfigurálható időablakon belül hasonlítsa össze. Csak egyértelmű találat esetén kapcsolja össze az eseményeket. Ha nincs találat vagy több lehetséges találat van, a program az eseményt ismeretlen folyamattal (`process: None`) jelenítse meg; ne találjon ki PID-et.

---

# 15.) feladat [4 pont]

Az `apppw-storage` crate-ben hozzon létre SQLite adatbázis-kezelést!

Az adatbázis legalább az alábbi táblákat tartalmazza:

```sql
processes
connections
http_requests
```

A táblákat az alkalmazás indulásakor `CREATE TABLE IF NOT EXISTS` utasításokkal lehet létrehozni. A Rust `Option<T>` mezőit SQL `NULL` értékként tárolja, amikor az érték `None`.

A `processes` táblában tárolja:

```text
id
pid
name
executable_path
```

A `connections` táblában:

```text
id
process_id
protocol
local_ip
local_port
remote_ip
remote_port
hostname
started_at
ended_at
bytes_sent
bytes_received
```

A `http_requests` táblában:

```text
id
connection_id
method
scheme
host
path
status
started_at
duration_ms
```

Írjon legalább:

```rust
pub fn save_process(&self, process: &ProcessInfo) -> Result<()>
pub fn save_connection(&self, connection: &NetworkConnection) -> Result<()>
pub fn save_http_request(&self, request: &HttpRequestInfo) -> Result<()>
```

metódusokat!

A metódusok fogadják a megfelelő `apppw-core` modellt, hajtsák végre az `INSERT` vagy szükség esetén `UPDATE` műveletet, és adjanak vissza `Result` értéket. Adatbázishibánál ne használjon `unwrap()` hívást, mert az leállítaná az egész alkalmazást.

---

# 16.) feladat [4 pont]

Az `apppw-ui` crate `src/main.rs` fájljában, illetve az abból meghívott modulokban készítsen `egui` / `eframe` alapú grafikus felületet!

Az ablak bal oldalán jelenítse meg a megfigyelt folyamatokat!

```text
Processes

● Discord.exe
  Steam.exe
  firefox.exe
  Game.exe
```

A folyamat kiválasztása után kizárólag annak hálózati aktivitása jelenjen meg!

Legyen:

```text
All processes
```

nézet is.

Az `eframe` alkalmazás állapotát egy saját `struct` tárolja, amely implementálja az `eframe::App` trait-et. A kijelölt folyamat lehet például `Option<u32>`: `None` jelentheti az „All processes” nézetet, `Some(pid)` pedig egy konkrét folyamatot.

---

# 17.) feladat [5 pont]

Készítsen hálózati eseménylistát!

A lista legalább az alábbi oszlopokat tartalmazza:

```text
Method
Host
Path
Status
Protocol
Sent
Received
Time
```

Például:

```text
GET    discord.com       /api/v10/users/@me       200   HTTPS   412 B   2.3 KB   74ms

POST   discord.com       /api/v10/channels/...    200   HTTPS   428 B   512 B    91ms

TLS    gateway.discord.gg                         ---   TLS     3.1 KB  8.4 KB   ---

UDP    162.159.130.234                             ---   UDP     3.2 KB  1.1 KB   ---
```

A WinDivertből érkező packet események ne blokkolják a grafikus felület frissítését! A háttérfeladat küldje az eseményeket channelen keresztül, a GUI pedig egy frissítés során csak a már megérkezett eseményeket olvassa ki. A GUI ne várakozzon blokkoló módon a következő csomagra.

---

# 18.) feladat [4 pont]

Készítsen részletes kérésnézetet!

HTTP kérés kiválasztásakor jelenjen meg:

```text
Headers
Request Body
Response
Timing
Connection
```

A `Connection` fülön jelenjen meg például:

```text
Process: Discord.exe
PID: 14280

Local:
192.168.1.10:53142

Remote:
162.159.135.232:443

Transport:
TCP

Captured using:
WinDivert
```

Az érzékeny HTTP headerek alapértelmezetten legyenek maszkolva! Ehhez a 11. feladat `redact_headers` függvényét használja; ne készítsen külön, eltérő maszkolási logikát a UI-ban.

---

# 19.) feladat [4 pont]

Valósítson meg keresést és szűrést!

A felhasználó szűrhessen:

```text
folyamatnév
host
HTTP metódus
státuszkód
protokoll
IP cím
port
```

alapján.

Példák:

```text
process:Discord.exe
```

```text
host:discord.com
```

```text
method:POST
```

```text
protocol:UDP
```

```text
port:443
```

Több feltétel egyidejű használata is legyen támogatott! A szóközzel elválasztott feltételek egyszerre legyenek igazak, vagyis logikai ÉS kapcsolatban álljanak. Például:

```text
process:Discord.exe protocol:TCP port:443
```

Az ismeretlen szűrőkulcs vagy hibás számérték ne okozzon `panic`-ot; a felület jelenítsen meg érthető hibaüzenetet.

---

# 20.) feladat [4 pont]

A program különböztesse meg a dekódolható és nem dekódolható eseményeket!

Használja legalább az alábbi állapotokat:

```text
HTTP
HTTPS decoded
TLS encrypted
TCP
UDP
QUIC
Unknown
```

Például:

```text
Game.exe

HTTPS decoded
POST api.game.example/player/inventory

TLS encrypted
auth.game.example:443

UDP
51.89.14.20:27015

Unknown
185.20.10.44:9000
```

A WinDivert által elfogott packet önmagában ne legyen HTTP/API kérésnek tekintve!

Ezek megjelenítési állapotok, nem feltétlenül új Rust enum-variánsok. A leképezés legyen következetes: dekódolt sima HTTP → `HTTP`, proxyn dekódolt HTTPS → `HTTPS decoded`, nem dekódolt TLS → `TLS encrypted`, azonosítatlan transportforgalom pedig a megfelelő `TCP`, `UDP`, `QUIC` vagy `Unknown` címkét kapja.

---

# 21.) feladat [5 pont]

Egészítse ki a programot valós idejű statisztikákkal!

A kiválasztott folyamat esetén jelenítse meg:

```text
Aktív kapcsolatok száma
HTTP kérések száma
Elfogott packetek száma
Összes elküldött adat
Összes fogadott adat
Különböző hostok száma
Átlagos HTTP válaszidő
```

Például:

```text
Discord.exe

Active connections:      18
Captured packets:        12,418
HTTP requests:           342
Uploaded:                4.2 MB
Downloaded:              39.7 MB
Hosts contacted:         14
Average response time:   83 ms
```

Minden értéket az aktuálisan kiválasztott folyamathoz és sessionhöz számoljon. Az „All processes” nézetben az összes folyamat adatait összegezze. Ha még nincs befejezett HTTP-kérés, az átlagos válaszidő helyén `---` jelenjen meg; ne osszon nullával.

---

# 22.) feladat [4 pont]

Valósítson meg session kezelést!

A felhasználó tudja elindítani és leállítani a rögzítést:

```text
Start Recording
Stop Recording
Clear
```

Minden session rendelkezzen:

```text
azonosítóval
kezdési időponttal
befejezési időponttal
kiválasztott folyamattal
```

Ehhez hozzon létre egy publikus `Session` modellt az `apppw-core` crate-ben. A kiválasztott folyamat legyen opcionális, mert az „All processes” módhoz nem tartozik egyetlen PID. A befejezési időpont rögzítés közben szintén legyen `None`.

A 15. feladat adatbázisát egészítse ki egy `sessions` táblával és a szükséges sessionazonosító idegen kulcsokkal. A korábbi sessionök SQLite adatbázisból legyenek visszatölthetők!

A `Clear` gomb az aktuális, még nem mentett képernyőnézetet ürítse. Korábban mentett adatbázisrekordokat csak külön, egyértelmű törlési művelet és megerősítés után szabad törölni.

---

# 23.) feladat [4 pont]

Készítsen JSON export funkciót!

Példa:

```json
{
  "process": {
    "pid": 14280,
    "name": "Discord.exe"
  },
  "connections": [
    {
      "remote": "162.159.135.232:443",
      "protocol": "TCP",
      "bytes_sent": 1024,
      "bytes_received": 8192
    }
  ],
  "requests": [
    {
      "method": "GET",
      "url": "https://discord.com/api/v10/users/@me",
      "status": 200,
      "duration_ms": 74
    }
  ]
}
```

Az exportált adatokban az érzékeny HTTP headerek alapértelmezetten ne szerepeljenek eredeti formában!

Az exportálható adatszerkezetekhez használható a `Serialize` derive, a JSON előállításához pedig a `serde_json`. Export előtt ugyanazt a `redact_headers` függvényt használja, mint a felületen. Ha a headerek nem részei az exportformátumnak, akkor ne adja hozzá őket csak az export kedvéért.

---

# 24.) feladat – komplex feladat [8 pont]

Kapcsolja össze az előző feladatokban létrehozott komponenseket!

Az alkalmazás indulását és a komponensek összekötését az `apppw-ui/src/main.rs` fájlból indítsa. A részletes logika külön modulokba kerülhet; a `main.rs` feladata elsősorban az inicializálás legyen.

A program indításakor:

1. kérdezze le a futó Windows folyamatokat;
2. indítsa el a WinDivert capture komponenst;
3. kezdje el feldolgozni a TCP és UDP packeteket;
4. építse fel a logikai kapcsolatokat;
5. rendelje a kapcsolatokat lehetőség szerint PID-hez és folyamathoz;
6. indítsa el az opcionális HTTP/HTTPS proxyt;
7. rendelje össze a proxy által megfigyelt HTTP requesteket a hálózati kapcsolatokkal;
8. mentse az eseményeket az adatbázisba;
9. továbbítsa őket a grafikus felületnek;
10. frissítse a megfelelő folyamat hálózati nézetét.

A komponensek között ne használjon globális módosítható állapotot!

Az események továbbítására használjon például:

```rust
tokio::sync::mpsc
```

csatornát. Ez hasonló egy Java blocking queue-hoz: az egyik komponens eseményt küld, egy másik pedig fogadja azt anélkül, hogy globális módosítható listán osztoznának.

A WinDivert capture loop külön worker threaden vagy aszinkron feldolgozási rétegen fusson.

A GUI bezárásakor:

```text
WinDivert handle
proxy
database worker
capture worker
background tasks
```

szabályosan álljanak le.

Ehhez küldjön leállítási jelzést a háttérfeladatoknak, zárja le a küldő csatornákat, majd várja meg a worker threadek vagy taskok befejeződését. A program ne hagyjon futó háttérfolyamatot maga után, és ne veszítsen el már adatbázisba küldött eseményt.

---

# A kész alkalmazás elvárt működése

A felhasználó kiválasztja:

```text
Discord.exe
```

folyamatot.

A program például:

```text
Discord.exe [PID: 14280]

NETWORK
──────────────────────────────────────────────────────────────

GET
discord.com/api/v10/users/@me
200 OK
74 ms

POST
discord.com/api/v10/channels/123/messages
200 OK
91 ms

TLS
gateway.discord.gg:443
Encrypted

UDP
162.159.130.234:50000
Unknown protocol
```

információkat jelenít meg.

Egy HTTP kérés kiválasztása után:

```text
POST https://discord.com/api/v10/channels/123/messages

GENERAL
────────────────────────
Process: Discord.exe
Status: 200 OK
Duration: 91 ms
Protocol: HTTPS

REQUEST HEADERS
────────────────────────
Content-Type: application/json
Authorization: [REDACTED]

REQUEST BODY
────────────────────────
{
    "content": "Hello!"
}

CONNECTION
────────────────────────
Remote: 162.159.135.232:443
Process ID: 14280
Protocol: TCP/TLS
Packet capture: WinDivert
```

---

# Javasolt fejlesztési sorrend

## Milestone 1 – Process Monitor

Készítse el:

```text
Windows process enumeration
PID
process name
executable path
egui process lista
```

---

## Milestone 2 – WinDivert Packet Capture

A program jelenítse meg:

```text
TCP
192.168.1.10:53142
→
162.159.135.232:443
```

és:

```text
UDP
192.168.1.10:52111
→
51.89.14.20:27015
```

kapcsolatokat.

Ezen a ponton még nincs szükség process attributionre.

---

## Milestone 3 – Process Attribution

A socket információk alapján jelenjen meg:

```text
Discord.exe
→ 162.159.135.232:443
```

ahelyett, hogy csak az IP és port lenne ismert.

---

## Milestone 4 – Traffic Accounting

A WinDivert packetek alapján számolja:

```text
sent bytes
received bytes
packet count
connection duration
```

értékeket.

---

## Milestone 5 – Network History

Tárolja:

```text
connection opened
connection updated
connection closed
```

eseményeket.

---

## Milestone 6 – HTTP Proxy

Jelenjenek meg a proxy által látott HTTP kérések:

```text
GET http://example.com/api/users
```

---

## Milestone 7 – HTTPS Inspection

Támogatott alkalmazások esetén jelenjen meg:

```text
GET https://example.com/api/users
```

Nem dekódolható kapcsolat esetén:

```text
TLS example.com:443
Encrypted
```

---

## Milestone 8 – WinDivert ↔ Process ↔ HTTP correlation

Az alkalmazás legyen képes összekötni:

```text
WinDivert packet
      ↓
NetworkConnection
      ↓
PID / Process
      ↓
HTTP request
```

adatokat.

Végső eredmény:

```text
Discord.exe

GET discord.com/api/v10/users/@me
```

---

## Milestone 9 – DevTools UI

Készüljön el:

- process lista,
- Network táblázat,
- request részletek,
- packet/connection részletek,
- szűrés,
- keresés,
- session recording.

---

## Milestone 10 – Persistence

Kerüljön bele:

- SQLite,
- korábbi sessionök,
- statisztikák,
- JSON export.

---

# Bónusz feladatok

## B1.) [2 pont]

Adjon lehetőséget `.har` formátumú HTTP session exportálására!

A HAR export csak a proxy által ténylegesen megfigyelt HTTP- és dekódolt HTTPS-kéréseket tartalmazza. A pusztán WinDiverttel látott titkosított TLS-, TCP- vagy UDP-forgalomból ne készítsen kitalált HAR-bejegyzést. Az érzékeny headereket itt is maszkolja.

---

## B2.) [3 pont]

Készítsen processenként valós idejű upload/download grafikont!

A grafikont a WinDivert által megfigyelt packetméretekből számolja!

A küldött és fogadott bájtokat külön idősorban gyűjtse, azonos időközönként összesítve. A GUI csak a már kiszámolt értékeket rajzolja ki; a packet capture továbbra se fusson a GUI threaden.

---

## B3.) [3 pont]

Készítsen DNS correlation modult!

A modul próbálja meg összekötni:

```text
DNS query
DNS response
IP address
későbbi TCP/UDP kapcsolat
```

adatokat.

Így például:

```text
162.159.135.232
```

helyett lehetőség szerint:

```text
gateway.discord.gg
```

jelenjen meg.

Csak a ténylegesen megfigyelt DNS-válaszokat használja, és vegye figyelembe, hogy egy IP-címhez több hostnév is tartozhat. Ha a kapcsolat nem rendelhető egyértelműen egy DNS-válaszhoz, maradjon az IP-cím; ne találjon ki hostnevet.

---

## B4.) [3 pont]

Készítsen WebSocket nézetet!

Jelenítse meg:

```text
URL
állapot
küldött frame-ek száma
fogadott frame-ek száma
```

értékeket.

WebSocket üzenetek csak akkor jeleníthetők meg, ha a proxy látja és dekódolja a kapcsolatot. Egy WinDivert által látott titkosított TLS-kapcsolatból önmagában nem állapíthatók meg a WebSocket frame-ek.

---

## B5.) [4 pont]

Készítsen Packet Inspector nézetet!

Megjeleníthető legyen:

```text
IPv4 / IPv6 header
TCP / UDP header
source
destination
flags
payload length
raw payload
```

A raw payload opcionálisan hexadecimális formában jelenjen meg.

A nyers payload tárolása és megjelenítése alapértelmezetten legyen kikapcsolva, mert érzékeny adatot tartalmazhat és sok memóriát használhat. Bekapcsolásakor korlátozza az egy csomagból eltárolt bájtok számát.

---

## B6.) [4 pont]

Készítsen konfigurálható WinDivert filtert!

Például:

```text
tcp.DstPort == 443
```

vagy:

```text
udp
```

vagy:

```text
outbound and tcp
```

A hibás filterkifejezést a program kezelje megfelelő hibaüzenettel!

Az új filtert alkalmazás előtt ellenőrizze. Hibás filter esetén tartsa meg az előző működő filtert, jelenítse meg a hibát a felületen, és ne állítsa le az alkalmazást.

---

# Fő architektúra

```text
                        ┌───────────────────────┐
                        │      Windows Apps     │
                        │ Discord / Game / etc. │
                        └───────────┬───────────┘
                                    │
                           Windows Network Stack
                                    │
                   ┌────────────────┴───────────────┐
                   │                                │
                   ▼                                ▼
            ┌─────────────┐                 ┌──────────────┐
            │  WinDivert  │                 │ HTTP Proxy   │
            │   Capture   │                 │ Hyper/rustls │
            └──────┬──────┘                 └──────┬───────┘
                   │                               │
                   ▼                               │
          Packet / Connection                      │
                   │                               │
                   ▼                               │
        ┌──────────────────────┐                   │
        │ SocketProcessResolver│                   │
        │ 5-tuple → PID        │                   │
        └──────────┬───────────┘                   │
                   │                               │
                   └──────────────┬────────────────┘
                                  ▼
                         ┌─────────────────┐
                         │TrafficCorrelator│
                         └────────┬────────┘
                                  │
              ┌───────────────────┼────────────────────┐
              ▼                   ▼                    ▼
           egui UI             SQLite              tracing
```

---

# Fontos architekturális szabály

A WinDivert packet capture és a process attribution **két külön feladat**.

A program ne feltételezze, hogy egy raw packet önmagában tartalmazza a PID-et!

A feldolgozás logikája ezért:

```text
WinDivert packet

src IP
src port
dst IP
dst port
protocol
        │
        ▼
5-tuple
        │
        ▼
Windows socket table
        │
        ▼
PID
        │
        ▼
ProcessInfo
```

Majd:

```text
NetworkConnection
        +
Proxy request
        ↓
TrafficCorrelator
        ↓
Discord.exe
POST discord.com/api/v10/...
```

---

# Összpontszám

```text
Alapfeladatok: 96 pont
Bónuszfeladatok: 19 pont
```

A 24 alapfeladat fejlécében szereplő pontértékek összege 96 pont. Ha az elvárt végösszeg mégis 100 pont, további 4 pontot valamelyik feladathoz külön hozzá kell rendelni.

A projekt akkor tekinthető alapvetően működőképesnek, ha:

- képes futó Windows folyamat kiválasztására;
- WinDivert segítségével képes TCP és UDP forgalmat megfigyelni;
- képes kapcsolatokat felépíteni a packetekből;
- lehetőség szerint PID-hez és alkalmazáshoz rendeli a kapcsolatokat;
- számolja az alkalmazásonkénti adatforgalmat;
- HTTP proxyként képes HTTP kérések megfigyelésére;
- támogatott esetben HTTPS kéréseket is képes megjeleníteni;
- összerendeli a HTTP kéréseket az alkalmazásokkal, amikor ez megbízhatóan megállapítható;
- a nem dekódolható forgalmat nem értelmezi tévesen HTTP kérésként;
- a felület valós időben frissül;
- az eseményeket sessionönként tárolja.

A végső cél:

> **„Milyen hálózati kapcsolatokat és – ahol megfigyelhető – milyen HTTP/API kéréseket kezdeményez ez a Windows alkalmazás?”**
