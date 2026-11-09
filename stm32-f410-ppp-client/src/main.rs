#![no_std]
#![no_main]

use defmt::{error, info, warn};
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_net::dns::DnsQueryType;
use embassy_net::tcp::TcpSocket;
use embassy_net::{
    Config as NetConfig, ConfigV4, IpAddress, IpEndpoint, Ipv4Address, Ipv4Cidr,
    Stack, StackResources, StaticConfigV4,
};
use minimq::{Buffers, ConfigBuilder, Publication, QoS, Session};
use embassy_stm32::usart::{self, BufferedUart, Config as UartConfig};
use embassy_stm32::{bind_interrupts, peripherals};
use embassy_time::{Duration, Timer};
use heapless::Vec;
use static_cell::StaticCell;

bind_interrupts!(struct Irqs {
    USART1 => usart::BufferedInterruptHandler<peripherals::USART1>;
});

static PPP_STATE: StaticCell<embassy_net_ppp::State<1, 1>> = StaticCell::new();
static STACK_RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
static UART_TX_BUFFER: StaticCell<[u8; 512]> = StaticCell::new();
static UART_RX_BUFFER: StaticCell<[u8; 512]> = StaticCell::new();

/* A deterministic seed is sufficient for this connectivity smoke test. It is
 * NOT a cryptographic entropy source and must not be reused for TLS keys/nonces. */
const NETWORK_SEED: u64 = 0xA9F3_4C21_7E5B_108D;

#[embassy_executor::task]
async fn network_runner_task(
    mut runner: embassy_net::Runner<'static, embassy_net_ppp::Device<'static>>,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn ppp_runner_task(
    mut runner: embassy_net_ppp::Runner<'static>,
    uart: BufferedUart<'static>,
    stack: Stack<'static>,
) -> ! {
    let ppp_config = embassy_net_ppp::Config {
        username: b"",
        password: b"",
    };

    let stack_for_callback = stack;
    let result = runner
        .run(uart, ppp_config, move |status| {
            let Some(local_address) = status.address else {
                warn!("PPP IPCP callback had no local IPv4 address");
                return;
            };

            let mut dns_servers: Vec<Ipv4Address, 3> = Vec::new();
            for server in status.dns_servers.iter().flatten() {
                let _ = dns_servers.push((*server).into());
            }

            /* Fallback DNS is useful for bring-up only. Prefer the DNS server
             * negotiated from the ESP32/upstream Wi-Fi network. */
            if dns_servers.is_empty() {
                let _ = dns_servers.push(Ipv4Address::new(1, 1, 1, 1));
            }

            let config = StaticConfigV4 {
                address: Ipv4Cidr::new(local_address.into(), 0),
                gateway: None,
                dns_servers,
            };
            stack_for_callback.set_config_v4(ConfigV4::Static(config));
            info!("PPP IPv4 configured");
        })
        .await;

    match result {
        Ok(never) => match never {},
        Err(_) => {
            error!("PPP runner terminated; reboot/restart the STM32 after fixing the link");
            loop {
                Timer::after_secs(5).await;
            }
        }
    }
}

#[embassy_executor::task]
async fn http_smoke_test(stack: Stack<'static>) -> ! {
    loop {
        info!("Resolving example.com");
        let addresses = match stack.dns_query("example.com", DnsQueryType::A).await {
            Ok(addresses) => addresses,
            Err(_) => {
                warn!("DNS query failed");
                Timer::after_secs(10).await;
                continue;
            }
        };

        let Some(destination) = addresses.first().copied() else {
            warn!("DNS returned no addresses");
            Timer::after_secs(10).await;
            continue;
        };

        let IpAddress::Ipv4(destination_v4) = destination;

        info!("DNS resolved; opening TCP port 80");
        let mut rx_buffer = [0u8; 512];
        let mut tx_buffer = [0u8; 512];
        let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(10)));

        if socket
            .connect(IpEndpoint::new(IpAddress::Ipv4(destination_v4), 80))
            .await
            .is_err()
        {
            warn!("TCP connection to example.com:80 failed");
            Timer::after_secs(10).await;
            continue;
        }

        info!("TCP connected; sending clear-text HTTP diagnostic request");
        let request = b"GET / HTTP/1.0\r\nHost: example.com\r\nConnection: close\r\n\r\n";
        let mut written = 0usize;
        while written < request.len() {
            match socket.write(&request[written..]).await {
                Ok(0) => break,
                Ok(count) => written += count,
                Err(_) => break,
            }
        }

        if socket.flush().await.is_err() {
            warn!("HTTP request flush failed");
        }

        let mut response = [0u8; 96];
        match socket.read(&mut response).await {
            Ok(count) => info!("HTTP diagnostic response bytes: {}", count as u32),
            Err(_) => warn!("HTTP response read failed"),
        }
        socket.close();

        info!("HTTP smoke test complete; repeating in 30 seconds");
        Timer::after_secs(30).await;
    }
}

#[embassy_executor::task]
async fn mqtt_smoke_test(stack: Stack<'static>) -> ! {
    //const BROKER_HOST: &str = "test.mosquitto.org";
    const BROKER_HOST: &str = "broker.hivemq.com";
    const BROKER_PORT: u16 = 1883;
    const TOPIC: &str = "stm32-f410-ppp/test";
    const PAYLOAD: &str = "hello from STM32 over PPP";

    let mut mqtt_rx_buffer = [0u8; 512];
    let mut mqtt_tx_buffer = [0u8; 1024];

    let config = ConfigBuilder::new(Buffers::new(
        &mut mqtt_rx_buffer,
        &mut mqtt_tx_buffer,
    ))
    .client_id("stm32-f410-ppp-absdskajbdsh")
    .unwrap();

    let mut session = Session::new(config);

    loop {
        info!("Resolving MQTT broker");
        let addresses = match stack.dns_query(BROKER_HOST, DnsQueryType::A).await {
            Ok(addresses) => addresses,
            Err(_) => {
                warn!("MQTT broker DNS query failed");
                Timer::after_secs(10).await;
                continue;
            }
        };

        let Some(destination) = addresses.first().copied() else {
            warn!("MQTT broker DNS returned no addresses");
            Timer::after_secs(10).await;
            continue;
        };

        let IpAddress::Ipv4(destination_v4) = destination;

        let mut tcp_rx_buffer = [0u8; 512];
        let mut tcp_tx_buffer = [0u8; 512];
        let mut socket = TcpSocket::new(stack, &mut tcp_rx_buffer, &mut tcp_tx_buffer);
        socket.set_timeout(Some(Duration::from_secs(10)));

        info!("Connecting to MQTT broker");
        if socket
            .connect(IpEndpoint::new(
                IpAddress::Ipv4(destination_v4),
                BROKER_PORT,
            ))
            .await
            .is_err()
        {
            warn!("MQTT broker TCP connection failed");
            Timer::after_secs(10).await;
            continue;
        }

        let mut connection = match session.connect(socket).await {
            Ok(connection) => connection,
            Err(err) => {
                warn!("MQTT CONNECT failed: {:?}", err);
                Timer::after_secs(10).await;
                continue;
            }
        };

        info!("Connected to MQTT broker; publishing test message");
        let op = match connection
            .publish(
                Publication::text(TOPIC, PAYLOAD).qos(QoS::AtLeastOnce),
            )
            .await
        {
            Ok(Some(op)) => op,
            Ok(None) => {

                warn!("MQTT publish returned no acknowledgment handle");
                let _ = connection.disconnect().await;
                Timer::after_secs(10).await;
                continue;
            }
            Err(_) => {
                warn!("MQTT publish failed");
                let _ = connection.disconnect().await;
                Timer::after_secs(10).await;
                continue;
            }
        };

        while connection.is_pending(&op) {
            if connection.poll().await.is_err() {
                warn!("MQTT publish acknowledgment failed");
                break;
            }
        }

        if connection.is_complete(&op) {
            info!("MQTT QoS 1 publish acknowledged by broker");
        }

        if connection.disconnect().await.is_err() {
            warn!("MQTT DISCONNECT failed");
        }
        
        info!("MQTT smoke test complete; repeating in 30 seconds");
        Timer::after_secs(30).await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    info!("STM32F410RB PPP client starting");

    let p = embassy_stm32::init(Default::default());

    let mut uart_config = UartConfig::default();
    uart_config.baudrate = 115_200;

    let uart = BufferedUart::new(
        p.USART1,
        p.PA10,
        p.PA9,
        UART_TX_BUFFER.init([0u8; 512]),
        UART_RX_BUFFER.init([0u8; 512]),
        Irqs,
        uart_config,
    )
    .unwrap();

    let ppp_state = PPP_STATE.init(embassy_net_ppp::State::<1, 1>::new());
    let (ppp_device, ppp_runner) = embassy_net_ppp::new(ppp_state);

    let resources = STACK_RESOURCES.init(StackResources::<3>::new());
    let (stack, net_runner) = embassy_net::new(
        ppp_device,
        NetConfig::default(),
        resources,
        NETWORK_SEED,
    );

    spawner.spawn(network_runner_task(net_runner)).unwrap();
    spawner
        .spawn(ppp_runner_task(ppp_runner, uart, stack))
        .unwrap();

    info!("PPP client started; waiting for IPv4 configuration");
    stack.wait_config_up().await;
    info!("PPP IP configuration is up");

    spawner.spawn(http_smoke_test(stack)).unwrap();
    spawner.spawn(mqtt_smoke_test(stack)).unwrap();
}
