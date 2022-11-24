use embassy_sync::blocking_mutex::Mutex;
use embedded_svc::ws::asynch::server::Acceptor;
use embedded_svc::ws::FrameType;
use esp_idf_hal::prelude::*;
use esp_idf_hal::task::embassy_sync::EspRawMutex;
use esp_idf_hal::task::executor::EspExecutor;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::http::server::ws::EspHttpWsProcessor;
use esp_idf_svc::http::server::EspHttpServer;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::{EspWifi, WifiWait};
use esp_idf_sys as _; // If using the `binstart` feature of `esp-idf-sys`, always keep this module imported
use log::info;

use core::cell::{Cell, RefCell};
mod web;
use web::*;
pub struct Config {
    wifi_ssid: &'static str,
    wifi_psk: &'static str,
    ws_max_con: usize,
    ws_max_frame_size: usize,
}

const CONFIG: Config = Config {
    wifi_ssid: "ENTER_YOUR_SSID",
    wifi_psk: "ENTER_YOUR_PW",
    ws_max_con: 2,
    ws_max_frame_size: 4096,
};

fn main() -> Result<(), ()> {
    esp_idf_sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take().unwrap();

    let sysloop = EspSystemEventLoop::take().unwrap();
    let nvs_default_partition = EspDefaultNvsPartition::take().unwrap();

    let mut wifi = EspWifi::new(
        peripherals.modem,
        sysloop.clone(),
        Some(nvs_default_partition),
    )
    .unwrap();

    wifi.set_configuration(&embedded_svc::wifi::Configuration::Client(
        embedded_svc::wifi::ClientConfiguration {
            ssid: CONFIG.wifi_ssid.into(),
            password: CONFIG.wifi_psk.into(),
            ..Default::default()
        },
    ))
    .unwrap();
    let wait = WifiWait::new(&sysloop).unwrap();
    let wifi = wifi.driver_mut();

    wifi.start().unwrap();
    wait.wait_with_timeout(std::time::Duration::from_secs(20), || {
        wifi.is_sta_started().unwrap()
    });
    wifi.connect().unwrap();
    wait.wait_with_timeout(std::time::Duration::from_secs(20), || {
        wifi.is_sta_connected().unwrap()
    });

    let (ws_processor, ws_acceptor) =
        EspHttpWsProcessor::<{ CONFIG.ws_max_con }, { CONFIG.ws_max_frame_size }>::new(());

    let ws_processor = Mutex::<EspRawMutex, _>::new(RefCell::new(ws_processor));
    let server_conf = esp_idf_svc::http::server::Configuration::default();
    let mut httpd = EspHttpServer::new(&server_conf).unwrap();

    httpd
        .ws_handler("/ws", move |connection| {
            ws_processor.lock(|ws_processor| ws_processor.borrow_mut().process(connection))
        })
        .unwrap();

    let executor = EspExecutor::<2, _>::new();
    let mut tasks = heapless::Vec::<_, 2>::new();

    executor
        .spawn_local_collect(ws_conn_handler(ws_acceptor), &mut tasks)
        .unwrap();

    info!("[MAIN] Starting Executor");
    executor.run_tasks(move || true, tasks);

    Ok(())
}

use embassy_sync::blocking_mutex::raw::{NoopRawMutex, RawMutex};
use embassy_sync::mutex::Mutex as AsyncMutex;
pub async fn ws_conn_handler<A: Acceptor>(acceptor: A) {
    loop {
        info!("[HANDLER] Wait for connection...");
        let (sender, mut receiver) = acceptor.accept().await.unwrap();
        info!("[HANDLER] ..got connection");
        let sender = AsyncMutex::<NoopRawMutex, _>::new(sender);

        let count_frames = AsyncMutex::<NoopRawMutex, _>::new(Cell::new(0_u32));

        let mut open = true;
        loop {
            if !open {
                break;
            }
            info!("[HANDLER] Wait for Frame..");
            open = receive(&mut receiver, &sender, &count_frames)
                .await
                .unwrap();
            info!("[HANDLER] ..finished receiving frame");
        }
    }
}

pub async fn receive(
    mut receiver: impl embedded_svc::ws::asynch::Receiver,
    sender: &AsyncMutex<impl RawMutex, impl embedded_svc::ws::asynch::Sender>,
    counter: &AsyncMutex<impl RawMutex, Cell<u32>>,
) -> Result<bool, ()> {
    let mut recv_buffer: [u8; 4096] = [0; 4096];
    let (frame_type, mut size) = receiver.recv(&mut recv_buffer).await.unwrap();
    size = if size > 0 { size } else { 1 };
    //let debug = core::str::from_utf8(&recv_buffer[..size-1]).unwrap();
    //info!("[RECEIVE] msg: {}", debug);

    let count = counter.lock().await;
    count.set(count.get() + 1);
    info!("[RECEIVE] Frame number: {:?}", count.get());

    let hold_open = match frame_type {
        FrameType::Text(_) => {
            let buffer: Result<WebRequest, serde_json::Error> =
                serde_json::from_slice(&recv_buffer[..size - 1]);
            let response = if let Ok(request) = buffer {
                info!("[RECEIVE] json: {:?}", request);
                match request {
                    WebRequest::Request => WebEvent::Event,
                    WebRequest::RequestWithPayload(_) => WebEvent::EventWithPayload(42),
                }
            } else {
                WebEvent::MalformedRequest
            };
            // create response only on one EventType to test if its hanging
            // only when answering on recv
            if WebEvent::EventWithPayload(42) == response {
                let msg = serde_json::to_vec(&response).unwrap();
                let msg_slice = msg.as_slice();
                info!("[RECEIVE] Frame {:?} Try Sending..", count.get());
                let mut sender_lock = sender.lock().await;
                sender_lock
                    .send(FrameType::Text(false), msg_slice)
                    .await
                    .unwrap();
                info!("[RECEIVE] Frame {:?} ..Send", count.get());
            }
            if WebEvent::Event == response {
                info!("[RECEIVE] Event -> Not sending Response");
            }
            true
        }
        FrameType::Binary(_) => true,
        FrameType::Continue(_) => true,
        FrameType::Ping => true,
        FrameType::Pong => true,
        FrameType::Close => false,
        FrameType::SocketClose => false,
    };
    Ok(hold_open)
}
