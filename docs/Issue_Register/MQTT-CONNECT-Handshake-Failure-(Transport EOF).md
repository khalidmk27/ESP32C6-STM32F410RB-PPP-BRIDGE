# STM32F410RB – MQTT CONNECT Handshake Failure (Transport EOF)

| Field | Details |
|---|---|
| **Issue ID** | STM32-MQTT-001 |
| **Component** | STM32F410RB – Embassy MQTT Client |
| **Category** | MQTT / TCP Transport / Broker Connectivity |
| **Severity** | Medium – MQTT diagnostic failure |
| **Status** | Resolved in current implementation |
| **Root Cause Confidence** | Broker-specific connection behaviour suspected; exact cause not conclusively established |
| **Affected Hardware** | STM32F410RB (NUCLEO-F410RB) |
| **MQTT Library** | `minimq 0.13.5` |

---

## 1. Issue Description

During initial MQTT connectivity testing over the ESP32-C6 ↔ STM32F410RB PPP bridge, the STM32 firmware repeatedly failed during the MQTT CONNECT handshake when connecting to the public Mosquitto test broker.

The HTTP smoke test was completing successfully, and the MQTT task was able to resolve the broker hostname and proceed to the connection stage. However, the MQTT connection was terminated before the handshake completed.

The following warnings were observed:

```text
[INFO ] HTTP smoke test complete; repeating in 30 seconds

[WARN ] Transport returned EOF during CONNECT
        (minimq mqtt_client/session/handshake.rs:115)

[WARN ] MQTT CONNECT failed
        (stm32_f410_ppp_client stm32-f410-ppp-client/src/main.rs:217)

[INFO ] Resolving MQTT broker

[INFO ] Connecting to MQTT broker

[WARN ] Transport returned EOF during CONNECT
        (minimq mqtt_client/session/handshake.rs:115)

[WARN ] MQTT CONNECT failed: DISCONNECT
        (stm32_f410_ppp_client stm32-f410-ppp-client/src/main.rs:217)
```

The HTTP smoke test continuing independently indicated that the PPP network connection was still operational while the MQTT diagnostic was experiencing connection failures.

The failure was therefore investigated at the MQTT connection and broker interaction level rather than treating it as a complete PPP connectivity failure.

## 2. Investigation and Root Cause Analysis

The investigation identified three relevant areas.

### 2.1 Public MQTT broker selection

The initial implementation used:

```rust
const BROKER_HOST: &str = "test.mosquitto.org";
const BROKER_PORT: u16 = 1883;
```

The firmware successfully progressed through DNS resolution and attempted to establish the MQTT connection, but the transport returned EOF during the CONNECT handshake.

This indicated that the MQTT session was not completing successfully.

The available logs did not establish whether the connection termination was caused by broker availability, broker-side connection handling, or another transport-level condition.

### 2.2 MQTT client ID

The MQTT configuration initially used a fixed client identifier.

MQTT clients connecting with the same client ID can interfere with one another when another active connection is using that identifier.

A distinct client ID was therefore assigned to the STM32 client to avoid potential conflicts with another running device or test process.

The current implementation uses:

```rust
.client_id("stm32-f410-ppp-absdskajbdsh")
.unwrap();
```

This is a hardcoded identifier for the current test configuration. It is not automatically generated or guaranteed to be unique across multiple flashed devices.

There was no conclusive evidence that a client ID collision caused the original EOF errors.

### 2.3 Insufficient MQTT error reporting

The earlier error-handling implementation discarded the actual error returned by `session.connect()`:

```rust
let mut connection = match session.connect(socket).await {
    Ok(connection) => connection,
    Err(_) => {
        warn!("MQTT CONNECT failed");

        Timer::after_secs(10).await;
        continue;
    }
};
```

Using `Err(_)` prevented the application from displaying the actual error value.

This made it difficult to distinguish between different MQTT connection failures.

The error handling was subsequently updated to capture and log the returned error.

## 3. Solution Implemented

The investigation resulted in three changes to the MQTT client configuration and diagnostics.

### 3.1 Changed the MQTT broker

The original broker hostname was replaced with HiveMQ's public MQTT broker.

**Previous configuration:**

```rust
const BROKER_HOST: &str = "test.mosquitto.org";
```

**Current configuration:**

```rust
// const BROKER_HOST: &str = "test.mosquitto.org";
const BROKER_HOST: &str = "broker.hivemq.com";

const BROKER_PORT: u16 = 1883;
```

The broker change was the reported working solution. Subsequent hardware testing demonstrated successful MQTT connectivity using this configuration.

### 3.2 Assigned a distinct MQTT client ID

The client ID was configured explicitly:

```rust
.client_id("stm32-f410-ppp-absdskajbdsh")
.unwrap();
```

This provides a distinct identifier for the STM32 test client and reduces the possibility of interference from another active MQTT connection using the same identifier.

For deployments involving multiple devices, the identifier should be generated or provisioned uniquely per device.

### 3.3 Improved MQTT error handling

The generic error handler was replaced with one that captures the actual error returned by the MQTT library.

**Updated implementation:**

```rust
let mut connection = match session.connect(socket).await {
    Ok(connection) => connection,
    Err(err) => {
        warn!("MQTT CONNECT failed: {:?}", err);

        Timer::after_secs(10).await;
        continue;
    }
};
```

This allows the firmware to display the underlying error value rather than reporting only a generic connection failure.

It also improves future debugging when investigating broker disconnections, MQTT handshake failures, or transport errors.

## 4. Verification and Results

The current repository implementation uses the HiveMQ broker, an explicit client ID, and detailed MQTT CONNECT error reporting.

The documented hardware runtime output demonstrates the following successful sequence:

```text
[INFO ] Resolving MQTT broker

[INFO ] Connecting to MQTT broker

[INFO ] Connected with a fresh broker session

[INFO ] Connected to MQTT broker; publishing test message

[INFO ] MQTT QoS 1 publish acknowledged by broker

[INFO ] Graceful disconnect requested

[INFO ] MQTT smoke test complete; repeating in 30 seconds
```

These results establish that:

1. MQTT broker hostname resolution succeeds.
2. The STM32 establishes a TCP connection to the broker.
3. The MQTT CONNECT handshake completes successfully.
4. The broker accepts the MQTT client session.
5. The STM32 publishes the test message using QoS 1.
6. The broker acknowledges the publication.
7. The client performs a graceful MQTT disconnection.

The successful QoS 1 acknowledgement confirms that the MQTT publish operation progressed beyond connection establishment.

## 5. Current MQTT Configuration

The current STM32 firmware uses the following MQTT configuration:

| Parameter | Current Value |
|---|---|
| MQTT library | `minimq 0.13.5` |
| Broker | `broker.hivemq.com` |
| TCP port | `1883` |
| Client ID | `stm32-f410-ppp-absdskajbdsh` |
| Topic | `stm32-f410-ppp/test` |
| Payload | `hello from STM32 over PPP` |
| Publish QoS | QoS 1 (`AtLeastOnce`) |
| MQTT transport | Plaintext TCP |
| Connection retry delay | 10 seconds |

The MQTT smoke test repeats its operation after a successful publish cycle, with a 30-second delay.

The implementation does not currently configure MQTT username/password authentication or TLS. It also does not implement MQTT subscription or application-level reply handling.

## 6. Final Resolution

The original MQTT CONNECT failures occurred when the STM32 PPP client attempted to communicate with `test.mosquitto.org`.

The working configuration replaced the original broker with `broker.hivemq.com`. The client ID was also explicitly configured, and MQTT error handling was improved to expose the actual error returned by `minimq`.

The current repository contains these changes and documents a successful hardware test in which the STM32 establishes an MQTT session, publishes a QoS 1 message, receives the broker acknowledgement, and disconnects gracefully.

The available evidence confirms that the MQTT connectivity problem was resolved in the tested configuration. However, the precise cause of the original broker connection termination remains unverified.

**Final status: RESOLVED in the current implementation.**

**Future consideration:** The current configuration uses a public MQTT broker over unencrypted TCP. Before extending this functionality beyond connectivity testing, implement broker authentication, TLS, and per-device client identifiers. Public broker availability and connection policies should also be considered when designing the final application.
