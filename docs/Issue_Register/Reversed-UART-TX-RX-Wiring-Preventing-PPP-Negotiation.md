# ESP32-C6 ↔ STM32F410RB – Reversed UART TX/RX Wiring Preventing PPP Negotiation

| Field | Details |
|---|---|
| **Issue ID** | BRIDGE-UART-001 |
| **Component** | ESP32-C6 ↔ STM32F410RB UART Interface |
| **Category** | Hardware Integration / UART Communication |
| **Severity** | High – PPP connection could not be established |
| **Status** | Resolved |
| **Root Cause Confidence** | Confirmed – Incorrect physical TX/RX connections |
| **Affected Hardware** | ESP32-C6 DevKitM-1 and STM32F410RB NUCLEO board |

---

## 1. Issue Description

During initial hardware integration of the ESP32-C6 PPP server and STM32F410RB PPP client, the two devices failed to establish a PPP connection over UART.

The STM32 firmware repeatedly reported that IPv4 configuration was unavailable:

```text
IPv4: DOWN
PPP client started; waiting for IPv4 configuration
```

The ESP32-C6 also remained waiting for the STM32 PPP client to establish communication and obtain an IP configuration.

Although both firmware applications were running, PPP negotiation did not progress to the stage where the STM32 could receive its IPv4 configuration.

The initial investigation focused on the UART communication link between the two development boards.

## 2. Investigation and Root Cause Analysis

The investigation identified an incorrect physical TX/RX connection between the ESP32-C6 and STM32F410RB.

The STM32 firmware was configured to use USART1 with:

- PA10 as USART1_RX.
- PA9 as USART1_TX.

The ESP32-C6 firmware was configured to use:

- GPIO21 as UART TX.
- GPIO20 as UART RX.

The initial wiring connected the corresponding TX and RX signals in the same direction instead of cross-connecting the transmitter of one device to the receiver of the other.

This prevented the two microcontrollers from correctly exchanging UART data required for PPP communication.

### Why this prevented PPP negotiation

UART communication between two devices requires a crossed TX/RX connection:

- The transmitter of the ESP32-C6 must connect to the receiver of the STM32F410RB.
- The receiver of the ESP32-C6 must connect to the transmitter of the STM32F410RB.
- Both boards must share a common ground reference.

With TX connected to TX and RX connected to RX, neither receiver gets the intended transmitted data from the other device.

Consequently, the PPP client and server could not exchange the frames required to establish the PPP session and negotiate an IPv4 address.

**Root cause:** Incorrect physical UART wiring. The TX and RX connections between the ESP32-C6 and STM32F410RB were reversed.

## 3. Solution Implemented

The STM32 firmware configuration was retained without modification.

The physical UART connections were corrected by swapping the TX/RX connections to establish the required cross-connection.

### Corrected UART wiring

| ESP32-C6 DevKitM-1 | STM32F410RB NUCLEO |
|---|---|
| GPIO21 (UART TX) | PA10 (USART1_RX) |
| GPIO20 (UART RX) | PA9 (USART1_TX) |
| GND | GND |

### STM32 firmware configuration

The existing USART1 configuration was retained:

```rust
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
```

The UART baud rate remained:

```rust
uart_config.baudrate = 115_200;
```

No changes to the UART peripheral configuration, GPIO assignments, or PPP implementation were required.

The correction was entirely at the physical wiring level.

## 4. Verification and Results

After correcting the TX/RX wiring, the ESP32-C6 and STM32F410RB were able to communicate over the UART interface.

The PPP connection subsequently progressed through negotiation and the STM32 obtained its IPv4 configuration.

The corrected wiring addressed the original communication failure without requiring changes to either firmware implementation.

The successful progression beyond the initial `IPv4: DOWN` state confirmed that the physical UART connection had been the issue.

## 5. Final Resolution

The PPP negotiation failure was caused by incorrectly connected UART TX/RX signals between the ESP32-C6 and STM32F410RB.

The STM32 firmware correctly configured USART1 with PA10 as RX and PA9 as TX. The problem was that the physical wiring did not follow the required cross-connection between the two devices.

The issue was resolved by correcting the wiring:

- ESP32 GPIO21 TX → STM32 PA10 RX.
- ESP32 GPIO20 RX → STM32 PA9 TX.
- ESP32 GND → STM32 GND.

The original firmware configuration was retained, and PPP communication was successfully established after the wiring correction.

**Final status: RESOLVED.**

**Engineering takeaway:** When debugging a PPP connection that remains in `IPv4: DOWN`, verify the physical UART TX/RX cross-connection and common ground before modifying the PPP stack, UART drivers, or network configuration.
