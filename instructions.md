# AppWatch - junior szintű projektleírás

## Mit készítesz?

Egy Windowsos Rust alkalmazást készítesz, amely megmutatja, hogy egy futó program milyen hálózati forgalmat használ. Például kiválaszthatod a `Discord.exe` vagy a `Steam.exe` folyamatot, és megnézheted a kapcsolatait, a küldött/fogadott adatmennyiséget, valamint ahol ez valóban lehetséges, a HTTP-kéréseit.

Nem kell minden adatot minden esetben felismerni. Ha egy adat nem állapítható meg biztosan, használd a megadott `None` vagy `Unknown` értéket. Ne találj ki PID-et, hostnevet vagy HTTP-kérést.

A programnak tudnia kell megkülönböztetni:

- TCP kapcsolat,
- UDP kapcsolat,
- HTTP kérés,
- HTTPS-en keresztül dekódolt HTTP kérés,
- nem dekódolható TLS kapcsolat,
- QUIC forgalom,
- ismeretlen vagy egyedi protokoll

értékeket.

## Mielőtt elkezded

Haladj sorrendben. Először az adatmodelleket és a folyamatlistát készítsd el, utána a csomagfigyelést, végül a proxy, adatbázis és felület részeit. Minden nagyobb lépés után futtasd a `cargo check` parancsot.

Hozz létre egy Rust workspace-t `AppWatch` néven:

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

Az `AppWatch/` és a `crates/` könyvtár. A gyökérben lévő `Cargo.toml` egy fájl. Minden `apppw-*` mappa külön Rust crate: saját `Cargo.toml` és `src/` mappája van.

A gyökérben lévő `Cargo.toml` workspace-manifest legyen, ne külön futtatható Rust package. Minimális tartalma:

```toml
[workspace]
resolver = "3"
members = ["crates/*"]
```

Az `apppw-core`, `apppw-windows`, `apppw-capture`, `apppw-proxy` és `apppw-storage` library crate, ezért a belépési fájljuk `src/lib.rs`. Az `apppw-ui` futtatható program, ezért a belépési fájlja `src/main.rs`.

## Egyszerű haladási terv

1. **Alapok (1-3. feladat):** Készítsd el az adatmodelleket: protokoll, folyamat, kapcsolat és címformázás.
2. **Windows adatok (4. és 6. feladat):** Listázd a folyamatokat, majd próbáld a socketeket folyamatokhoz kötni.
3. **Csomagok és kapcsolatok (5., 7-9. feladat):** Figyeld a TCP/UDP csomagokat, alakítsd őket eseménnyé, és számold a kapcsolat forgalmát.
4. **HTTP adatok (10-14. feladat):** Írd le a HTTP-kérések modelljét, takard ki a titkos headereket, majd készíts proxyt és korrelációt.
5. **Mentés és felület (15-23. feladat):** Ments SQLite-ba, készíts listát, szűrést, részletező nézetet, sessionöket és JSON exportot.
6. **Összekötés (24. feladat):** Indítsd el és állítsd le rendezetten az összes komponenst.

Az alábbi részletes feladatleírás a pontos típusneveket, mezőket és elvárt működést tartalmazza. Ezeket tartsd meg akkor is, ha a saját kódodat több kisebb modulra bontod.

## Fontos Rust fogalmak

Ez a rövid útmutató akkor is elég, ha Java után most ismerkedsz Rusttal.

- A `crate` egy külön fordítható Rust-projekt. Java esetén nagyjából egy Maven- vagy Gradle-modulnak felel meg.
- A `struct` adatokat tárol, hasonló egy egyszerű Java-osztályhoz. A hozzá tartozó metódusok az `impl TípusNév` blokkba kerülnek.
- Az `enum` többféle állapotot tud leírni, és szükség esetén adatot is tárolhat.
- A `trait` egy szabály vagy szerződés, Java esetén interface-hez hasonlítható. A `#[derive(...)]` több trait implementációját automatikusan elkészíti.
- Az `Option<T>` azt jelenti, hogy lehet érték (`Some(value)`), de hiányozhat is (`None`).
- A `Result<T, E>` siker esetén `Ok(value)`, hiba esetén `Err(error)`. Ha rövid `Result<T>` alakot használsz, válassz konkrét hibát vagy például `anyhow::Result<T>` aliast.
- A `Vec<T>` egy bővíthető lista, Java esetén `ArrayList<T>`.
- A `String` saját tulajdonú szöveg, a `&str` egy szövegre mutató kölcsönzött nézet.
- A `Self` az éppen megvalósított típust, a `&self` pedig egy létező példány elérését jelenti.
- A mintákban lévő `...` csak kihagyott kódrészletet jelez; ne másold be Rust-kódba.

A feladatban név szerint kért és más crate-ekből használt típusok, függvények legyenek `pub` értékűek. Ennél a tanulóprojektnél a felsorolt modellmezők is lehetnek `pub` mezők, gettereket nem kell írnod. Ha egy crate egy másik crate típusát használja, add hozzá path dependencyként a használó crate `Cargo.toml` fájljához:

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

# Külső crate-ek - kezdő gyorstalpaló

Az alábbi rövid példák megmutatják, hogyan indíthatod el az adott crate használatát. Nem teljes megoldások, csak kiindulópontok.

Fontos: egy dependency mindig abba a crate-be kerüljön, amelyik közvetlenül használja. Ne tedd automatikusan az összeset a workspace gyökér-manifestjébe.

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

# Feladatok junior nyelven

Az alábbi lista azt mondja el egyszerűen, mi a célja az egyes feladatoknak. A közvetlenül utána következő részben találod a pontos mezőket, típusneveket és függvényaláírásokat. A pontos neveket ne változtasd meg.

1. **Protokollok:** Készíts két `enum` típust: az egyik a hálózati protokollt, a másik a forgalom irányát írja le. Tedd őket menthetővé Serde-del.
2. **Folyamatmodell:** Készíts egy `ProcessInfo` adatmodellt PID-del, névvel és opcionális programútvonallal. A `Display` alakja legyen könnyen olvasható.
3. **Kapcsolatmodell:** Írd le egy hálózati kapcsolat helyi és távoli végpontját, folyamatát és számlálóit. IPv6 címnél is helyes `cím:port` formátumot használj.
4. **Folyamatlista:** A Windows API-val olvasd ki a futó programokat. Egyetlen elérhetetlen folyamat ne állítsa le a teljes listázást.
5. **Csomagfigyelés:** WinDiverttel külön háttérszálon figyeld a TCP és UDP csomagokat. A csomagokat alapból csak figyeld, ne módosítsd.
6. **PID hozzárendelés:** A Windows socket táblák alapján próbáld a kapcsolatot egy folyamathoz kötni. Bizonytalan UDP találat esetén eredmény `None`.
7. **Események és követés:** Hozz létre eseményt a csomagokról és kapcsolatokról. Azonos 5-tuple-höz tartozó csomagokat ugyanahhoz a logikai kapcsolathoz tartsd.
8. **Kapcsolat állapota:** Új kapcsolatnál nyitási, változásnál frissítési, régi/inaktív kapcsolatnál lezárási eseményt küldj. A két irány bájtjai ugyanahhoz a kapcsolathoz kerüljenek.
9. **Csomagmodell:** Tárold a csomag időpontját, irányát, címeit, portjait, méretét és opcionális folyamatát. A nyers adat tárolása legyen kikapcsolva alapból.
10. **HTTP modell:** Készíts típusokat HTTP-kéréshez és headerhez. A kérés és válasz méretét akkor is mentsd, ha a body tartalmát nem őrzöd meg.
11. **Titkok takarása:** Másolat készítése közben rejtsd el a jelszószerű HTTP headereket. A headernév kis- és nagybetűje ne számítson.
12. **HTTP proxy:** Indíts helyi proxyt, amely fogadja, továbbítja és rögzíti a HTTP kérést és választ. A proxyhiba legyen valódi hiba, ne sikeres válasz.
13. **HTTPS proxy:** Csak a felhasználó által proxyra beállított és a helyi tanúsítványt elfogadó alkalmazások HTTPS forgalmát dekódold. Certificate pinninget ne kerülj meg.
14. **Adatok összekapcsolása:** Próbáld a csomagot, a socket/PID adatot és a proxykérést biztonságosan összekötni. Csak egyértelmű találatot fogadj el.
15. **Adatbázis:** SQLite-ba ments folyamatokat, kapcsolatokat és HTTP-kéréseket. Hibánál adj vissza `Result` értéket, ne használj `unwrap()`-ot.
16. **Folyamatlista a felületen:** Egui felületen válassz ki egy folyamatot, vagy jeleníts meg minden folyamatot.
17. **Hálózati lista:** Mutasd táblázatban a fontos hálózati és HTTP adatokat. A háttérből érkező események ne állítsák meg a felületet.
18. **Részletező nézet:** Kérés kiválasztásakor jelenítsd meg a headereket, body-kat, időzítést és kapcsolatot. A header maszkolásához a közös függvényt használd.
19. **Szűrés:** Támogasd a megadott `kulcs:érték` kereséseket és az egyszerre több feltételt. Hibás keresésnél írj ki érthető üzenetet, ne álljon le a program.
20. **Jelölések:** A felület különböztesse meg a HTTP-t, dekódolt HTTPS-t, titkosított TLS-t, TCP-t, UDP-t, QUIC-ot és ismeretlen forgalmat.
21. **Statisztikák:** A kiválasztott folyamatra vagy az összesre számold a kapcsolatokat, csomagokat, adatforgalmat, hostokat és átlagos HTTP választ.
22. **Sessionök:** A felhasználó indíthasson, állíthasson le és üríthessen rögzítést. A korábbi mentett rekord csak külön, megerősített törléssel törölhető.
23. **JSON export:** Exportáld a session fontos adatait JSON-ba. Érzékeny HTTP header ne jelenjen meg eredeti értékkel.
24. **Teljes alkalmazás:** Indítsd el, kösd össze, majd bezáráskor rendezetten állítsd le az összes komponenst. Ne használj globális, módosítható állapotot.

---

# 1.) feladat [2 pont]

Az `apppw-core/src/lib.rs` fájlban készíts egy publikus `NetworkProtocol` enumot. A deklaráció kezdete legyen `pub enum NetworkProtocol`, és az alábbi értékeket tartalmazza:

```rust
TCP
UDP
HTTP
HTTPS
TLS
QUIC
UNKNOWN
```

Ugyanitt készíts egy publikus `ConnectionDirection` enumot is ezekkel az értékekkel:

```rust
Inbound
Outbound
```

Mindkét enum kapja meg `derive`-dal ezeket a trait-eket:

```rust
Debug
Clone
Copy
PartialEq
Eq
Serialize
Deserialize
```

A `Serialize` és `Deserialize` miatt add hozzá a `serde` crate-et az `apppw-core/Cargo.toml` fájlhoz `derive` feature-rel, majd importáld a két trait-et a forrásban.

---

# 2.) feladat [2 pont]

Az `apppw-core/src/lib.rs` fájlban készíts egy publikus `ProcessInfo` structot. Ez egy folyamat adatait tartja. Használj `impl ProcessInfo` blokkot a metódusaihoz.

A struct mezői legyenek:

```rust
pid: u32
name: String
executable_path: Option<PathBuf>
```

- `pid`: a folyamat azonosítója;
- `name`: a folyamat neve, például `Discord.exe`;
- `executable_path`: a futtatható fájl útvonala, vagy `None`, ha az nem olvasható ki.

## Konstruktor

Készíts az `impl ProcessInfo` blokkban egy publikus `new` függvényt ezzel az aláírással:

```rust
pub fn new(
    pid: u32,
    name: String,
    executable_path: Option<PathBuf>,
) -> Self
```

A `new` adjon vissza egy `ProcessInfo` értéket, és mentse el mindhárom paramétert a vele azonos nevű mezőbe.

Példa a konstruktor használatára:

```rust
let process = ProcessInfo::new(
    14280,
    String::from("Discord.exe"),
    None,
);
```

## Szöveges megjelenítés (`Display`)

Valósítsd meg a `std::fmt::Display` trait-et is. Ez mondja meg, mi jelenjen meg `println!("{}", process)` használatakor:

```rust
println!("{}", process);
```

A kimenet pontosan ez legyen:

```text
Discord.exe [PID: 14280]
```

Használd ezt a formátumot:

```text
<name> [PID: <pid>]
```

Az `executable_path` ne szerepeljen a kiírásban.

---

# 3.) feladat [3 pont]

Az `apppw-core` crate-ben készíts egy publikus `NetworkConnection` structot. Minden felsorolt mező legyen `pub`, mert más crate-ek is használják:

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

Az `impl` blokkban készítsd el ezt a metódust:

```rust
pub fn remote_address(&self) -> String
```

metódust!

Ez a távoli IP-címet és portot adja vissza, például:

```text
162.159.135.232:443
```

Készítsd el ezt a metódust is:

```rust
pub fn display_destination(&self) -> String
```

metódust is!

Ha van `hostname`, azt használd, például:

```text
discord.com:443
```

Ha nincs hostname, az IP-címet használd. IPv6-nál a formátum legyen helyes, például `[::1]:443`. Ehhez használhatod a szabványos `SocketAddr` típust, nem kell kézzel szöveget összeraknod.

---

# 4.) feladat [3 pont]

Az `apppw-windows` crate-ben készíts egy publikus `ProcessCollector` structot. Ebben legyen ez a metódus:

```rust
pub fn list_processes(&self) -> Result<Vec<ProcessInfo>>
```

Válassz egy konkrét hibatípust, például `windows::core::Result` vagy `anyhow::Result`, és használd következetesen. Siker esetén `Ok(processes)`, teljes lekérdezési hibánál `Err(error)` legyen az eredmény.

A metódus olvassa ki az éppen futó Windows folyamatokat. Minden elérhető folyamatnál mentsd el:

- PID,
- folyamat nevét,
- futtatható állomány útvonalát.

Használhatod a `windows` crate-et. Egy folyamat hibája ne állítsa le a teljes listát: hagyd ki, vagy tárold az ismeretlen útvonalat `None` értékkel.

---

# 5.) feladat [4 pont]

Az `apppw-capture` crate-ben készíts egy publikus `PacketCapture` structot, amely WinDiverttel figyeli a Windows hálózati forgalmát.

Nyiss WinDivert handle-t TCP és UDP csomagokra. A végleges szűrő mindkét irányt engedje át, mert később a küldött és fogadott bájtokat is számolod:

```text
(tcp or udp)
```

Az `outbound and (tcp or udp)` csak kezdeti próba lehet, mert nem látja a fogadott adatokat.

Minden csomagból legalább ezeket olvasd ki:

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

Hozz létre belső hálózati eseményt a csomag adataiból. A megfigyelt csomagot ne módosítsd. Ha a WinDivert módja visszainjektálást kér, változatlanul küldd vissza.

A csomagolvasó ciklus külön worker threaden vagy háttérfeladatban fusson, hogy a felület ne akadjon meg.

---

# 6.) feladat [4 pont]

Az `apppw-windows` crate-ben készíts egy publikus `SocketProcessResolver` structot. Ez próbálja meg egy hálózati kapcsolatot Windows folyamathoz kötni a TCP és UDP socketlisták alapján.

TCP-nél ezeket hasonlítsd össze:

```text
local IP
local port
remote IP
remote port
protocol
```

TCP-nél a Windows tábla helyi/távoli címet, portot és PID-et ad. UDP-nél csak a helyi címet, portot és PID-et, ezért UDP-nél csak a protokoll és helyi végpont alapján keress. Ha egy UDP porthoz több folyamat is illik, az eredmény `None`.

Hivatalos referencia: [MIB_TCPROW_OWNER_PID](https://learn.microsoft.com/en-us/windows/win32/api/tcpmib/ns-tcpmib-mib_tcprow_owner_pid) és [MIB_UDPROW_OWNER_PID](https://learn.microsoft.com/en-us/windows/win32/api/udpmib/ns-udpmib-mib_udprow_owner_pid).

Siker esetén a kapcsolat PID-hez, majd `ProcessInfo` értékhez jut:

```text
PID → ProcessInfo
```

Legyen egy publikus metódusod, amely `Option<ProcessInfo>` értéket ad. Találatkor `Some(process_info)`, bizonytalan vagy hibás esetben `None`. A metódus nevét te választod.

Például:

```text
Discord.exe [PID: 14280]

192.168.1.10:53142
→
162.159.135.232:443
```

Ha nem vagy biztos a folyamatban, ne találj ki PID-et. Ilyenkor használd ezt:

```rust
process: None
```

értéket használjon!

---

# 7.) feladat [4 pont]

Az `apppw-core` crate-ben készíts egy publikus `NetworkEvent` enumot ezekkel az értékekkel:

```rust
PacketCaptured(CapturedPacket),
ConnectionOpened(NetworkConnection),
ConnectionUpdated(NetworkConnection),
ConnectionClosed(u64)
```

Itt valódi `CapturedPacket` típust használj, ne `...` jelet. A 7. és 9. feladatot együtt is elkészítheted.

Az `apppw-capture` crate-ben készíts egy publikus `ConnectionTracker` structot. Ez a WinDivert által látott TCP/UDP csomagokból tartja nyilván a kapcsolatokat.

Egy kapcsolat azonosítója az 5-tuple:

```text
protocol
source IP
source port
destination IP
destination port
```

Kapcsolatonként legalább ezeket tartsd nyilván:

```text
kapcsolat kezdete
utolsó aktivitás
elküldött bájtok
fogadott bájtok
folyamat
```

Ezek lehetnek a tracker belső rekordjában; nem kötelező mindet a `NetworkConnection` structba tenni.

---

# 8.) feladat [3 pont]

A `ConnectionTracker` ismerje fel az új TCP kapcsolatot. Új kapcsolatnál küldje ezt az eseményt:

```rust
NetworkEvent::ConnectionOpened(connection)
```

eseményt.

Már ismert kapcsolatnál frissítsd ezeket:

```text
bytes_sent
bytes_received
last_seen
```

értékeket.

Lezárt vagy rég inaktív kapcsolatnál küldd ezt:

```rust
NetworkEvent::ConnectionClosed(connection_id)
```

eseményt.

UDP-nél az azonos 5-tuple-höz tartozó csomagokat kezelheted egy logikai sessionként.

Az `A -> B` és a válasz `B -> A` ugyanaz a kapcsolat legyen. A helyi végpont alapján döntsd el, hogy a bájtok a `bytes_sent` vagy a `bytes_received` számlálót növelik. Az inaktivitási időkorlát legyen névvel ellátott, könnyen módosítható konstans.

---

# 9.) feladat [4 pont]

Az `apppw-core` crate-ben készíts egy publikus `CapturedPacket` structot. Legalább ezek a mezői legyenek:

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

Alakítsd a WinDivert csomagját `CapturedPacket` értékké. A `packet_size` a teljes méret, a `payload_size` csak a fejlécek utáni hasznos adat mérete, ezért eltérhetnek.

A nyers csomag tárolása legyen opcionális, például `raw_packet: Option<Vec<u8>>`. Alapértéke `None`, hogy a program ne fogyasszon sok memóriát.

---

# 10.) feladat [3 pont]

Az `apppw-core` crate-ben készíts egy publikus `HttpRequestInfo` structot ezekkel a mezőkkel:

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

A body méretét akkor is töltsd ki, ha magát a tartalmat memória- vagy adatvédelmi okból nem mented el. Az `Option` itt azt jelenti, hogy az adat még nem érkezett meg, nem ismert, vagy szándékosan nincs tárolva.

Ugyanitt készíts `HttpHeader` structot ezekkel a mezőkkel:

```rust
name: String
value: String
```

Készítsd el ezt a metódust is:

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

Az `apppw-core` crate-ben írd meg ezt a publikus függvényt:

```rust
pub fn redact_headers(headers: &[HttpHeader]) -> Vec<HttpHeader>
```

függvényt!

Ezeknek az érzékeny headereknek az értékét:

```text
Authorization
Proxy-Authorization
Cookie
Set-Cookie
```

cseréld erre:

```text
[REDACTED]
```

értékre. A headernév kis- és nagybetűje ne számítson. A bemeneti slice-ot ne módosítsd: adj vissza új `Vec<HttpHeader>` értéket, és csak abban írd át az érzékeny adatokat.

---

# 12.) feladat [5 pont]

Az `apppw-proxy` crate-ben készíts egy publikus `HttpProxy` structot. A proxy egy beállítható helyi TCP porton figyeljen. Az alapértelmezett cím:

```text
127.0.0.1:8877
```

A proxy fogadjon és továbbítson HTTP-kéréseket. A kérésből mentsd el:

```text
HTTP method
host
path
headers
body size
```

A válaszból mentsd el:

```text
status code
headers
body size
response time
```

Minden kérésből hozz létre `HttpRequestInfo` értéket. A proxy kliensként kapcsolódjon a célkiszolgálóhoz, küldje tovább a kérést, majd adja vissza a választ az eredeti kliensnek. A proxyhiba ne legyen sikeres HTTP-válasz; adj értelmes hibát.

Ebben a feladatban a proxy még nem biztos, hogy tudja a folyamatot. Ilyenkor `HttpRequestInfo.process` legyen `None`; ezt a 14. feladat oldja meg.

Használható:

```text
Tokio
Hyper
hyper-util
http-body-util
```

---

# 13.) feladat [5 pont]

Egészítsd ki az `apppw-proxy`-t HTTPS támogatással.

TLS kommunikációhoz használhatja:

```text
rustls
tokio-rustls
```

Tanúsítványok létrehozásához használhatja:

```text
rcgen
```

Csak olyan alkalmazás HTTPS-forgalmát dekódold, amely a beállított helyi proxyt használja és elfogadja a tanúsítványát.

Ha egy kapcsolat nem dekódolható, akkor a WinDivert-alapú capture komponens továbbra is jelenítse meg például:

```text
Discord.exe

TLS
162.159.135.232:443

Contents unavailable
```

Ne próbáld megkerülni a certificate pinninget. Ez csak tudatosan helyi proxyra beállított alkalmazásokhoz való funkció. A helyi CA privát kulcsát ne exportáld és ne naplózd. A nem támogatott TLS kapcsolat maradjon titkosított kapcsolat, ne jelenjen meg hibás HTTP-kérésként.

---

# 14.) feladat [5 pont]

Az `apppw-core` crate-ben készíts egy publikus `TrafficCorrelator` structot. Ez három adatforrást próbál összekötni:

```text
WinDivert packet
        +
Windows socket/process information
        +
HTTP proxy event
```

Az összekötéshez ezeket vizsgáld:

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

Az időpontokat egy kicsi, beállítható időablakban hasonlítsd. Csak egyértelmű találatnál kösd össze az adatokat. Ha nincs vagy több találat van, használd a `process: None` értéket; ne találj ki PID-et.

---

# 15.) feladat [4 pont]

Az `apppw-storage` crate-ben készíts SQLite adatbázis-kezelést. Legalább ezek a táblák legyenek benne:

```sql
processes
connections
http_requests
```

A táblákat induláskor `CREATE TABLE IF NOT EXISTS` utasításokkal hozd létre. A `None` érték SQL-ben `NULL` legyen.

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

Legalább ezek a metódusok legyenek meg:

```rust
pub fn save_process(&self, process: &ProcessInfo) -> Result<()>
pub fn save_connection(&self, connection: &NetworkConnection) -> Result<()>
pub fn save_http_request(&self, request: &HttpRequestInfo) -> Result<()>
```

metódusokat!

A metódusok a megfelelő `apppw-core` modellt kapják, `INSERT`-et vagy szükség esetén `UPDATE`-et hajtanak végre, és `Result` értéket adnak vissza. Adatbázishibánál ne használj `unwrap()`-ot.

---

# 16.) feladat [4 pont]

Az `apppw-ui` `src/main.rs` fájljában vagy az onnan hívott modulokban készíts `egui` / `eframe` felületet. Az ablak bal oldalán jelenjenek meg a figyelt folyamatok:

```text
Processes

● Discord.exe
  Steam.exe
  firefox.exe
  Game.exe
```

A kiválasztott folyamatnál csak annak hálózati aktivitását mutasd. Legyen ilyen nézet is:

```text
All processes
```

Az alkalmazás állapotát saját struct tárolja, amely megvalósítja az `eframe::App` trait-et. A kijelölés lehet `Option<u32>`: `None` az `All processes`, `Some(pid)` egy konkrét folyamat.

---

# 17.) feladat [5 pont]

Készíts hálózati eseménylistát. Legalább ezek az oszlopok legyenek benne:

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

A WinDivert eseményei ne akadasszák meg a felületet. A háttérfeladat channelen küldje az eseményeket, a GUI pedig frissítéskor csak a már megérkezetteket olvassa ki. A GUI ne várjon a következő csomagra.

---

# 18.) feladat [4 pont]

Készíts részletes kérésnézetet. HTTP-kérés kiválasztásakor ezek jelenjenek meg:

```text
Headers
Request Body
Response
Timing
Connection
```

A `Connection` fülön például ez látszódjon:

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

Az érzékeny HTTP headerek alapból legyenek maszkolva. Ehhez a 11. feladat `redact_headers` függvényét használd, ne írj külön UI-s maszkolást.

---

# 19.) feladat [4 pont]

Készíts keresést és szűrést. A felhasználó ezek alapján szűrhessen:

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

Több feltétel is működjön egyszerre. A szóközzel elválasztott feltételek között logikai ÉS kapcsolat legyen, például:

```text
process:Discord.exe protocol:TCP port:443
```

Ismeretlen szűrőkulcs vagy hibás szám ne okozzon `panic`-ot; jeleníts meg érthető hibaüzenetet.

---

# 20.) feladat [4 pont]

A program különböztesse meg a dekódolható és nem dekódolható eseményeket. Legalább ezeket a címkéket használd:

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

Egy WinDivert csomag önmagában nem bizonyít HTTP/API-kérést. Ezek megjelenítési címkék, nem kötelező új Rust enumok. Használd őket következetesen: dekódolt HTTP -> `HTTP`, proxyval dekódolt HTTPS -> `HTTPS decoded`, nem dekódolt TLS -> `TLS encrypted`, egyéb forgalom -> `TCP`, `UDP`, `QUIC` vagy `Unknown`.

---

# 21.) feladat [5 pont]

Egészítsd ki a programot valós idejű statisztikákkal. A kiválasztott folyamatnál jelenjen meg:

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

Minden érték az aktuálisan kiválasztott folyamatra és sessionre vonatkozzon. `All processes` nézetben összegezd az összes folyamatot. Ha nincs befejezett HTTP-kérés, az átlagos válaszidő legyen `---`; ne ossz nullával.

---

# 22.) feladat [4 pont]

Készíts session kezelést. A felhasználó indíthassa és állíthassa le a rögzítést:

```text
Start Recording
Stop Recording
Clear
```

Minden sessionnek legyen:

```text
azonosítóval
kezdési időponttal
befejezési időponttal
kiválasztott folyamattal
```

Készíts publikus `Session` modellt az `apppw-core` crate-ben. A kiválasztott folyamat opcionális, mert `All processes` nézetben nincs egy PID. Rögzítés közben a befejezési idő `None`.

A 15. feladat adatbázisát bővítsd `sessions` táblával és a szükséges sessionazonosító idegen kulcsokkal. A korábbi sessionök tölthetők legyenek vissza SQLite-ból.

A `Clear` csak az aktuális, még nem mentett képernyőadatot ürítse. Mentett adatbázisrekordot csak külön, egyértelmű törléssel és megerősítés után törölj.

---

# 23.) feladat [4 pont]

Készíts JSON exportot.

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

Az érzékeny HTTP headerek ne kerüljenek eredeti értékkel az exportba. Az exportálható típusokhoz használhatod a `Serialize` derive-ot, JSON-hoz a `serde_json`-t. Export előtt ugyanazt a `redact_headers` függvényt használd, mint a felületen. Ha a headerek nem részei az exportformátumnak, ne add hozzá őket csak emiatt.

---

# 24.) feladat – komplex feladat [8 pont]

Kösd össze az előző feladatok komponenseit. Az indítást és az összekötést az `apppw-ui/src/main.rs` indítsa; a részletes logika külön modulokban lehet. Induláskor történjen ez:

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

Ne használj globális, módosítható állapotot. Az események továbbításához használhatsz például:

```rust
tokio::sync::mpsc
```

csatornát: az egyik komponens küld, a másik fogad, közös módosítható lista nélkül. A WinDivert ciklus külön worker threaden vagy aszinkron háttérben fusson.

A GUI bezárásakor ezek álljanak le rendezetten:

```text
WinDivert handle
proxy
database worker
capture worker
background tasks
```

szabályosan álljanak le.

Küldj leállítási jelzést a háttérfeladatoknak, zárd le a küldő csatornákat, majd várd meg a workerek és taskok végét. Kilépés után ne maradjon háttérfolyamat, és ne vesszen el már adatbázisba küldött esemény.

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
