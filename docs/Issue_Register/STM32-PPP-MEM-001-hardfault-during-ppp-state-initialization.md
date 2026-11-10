# STM32F410RB – HardFault During PPP State Initialization

| Field                     | Details                                                             |
| ------------------------- | ------------------------------------------------------------------- |
| **Issue ID**              | STM32-PPP-MEM-001                                                   |
| **Component**             | STM32F410RB – Embassy PPP Client                                    |
| **Category**              | Memory Management / Initialization                                  |
| **Severity**              | High – Firmware startup failure                                     |
| **Status**                | Resolved in current implementation                                  |
| **Root Cause Confidence** | Suspected memory pressure;                                          |
| **Affected Hardware**     | STM32F410RB (NUCLEO-F410RB)                                         |

## 1. Issue Description

During the initial development and testing of the STM32F410RB PPP client, the firmware repeatedly encountered a HardFault while initializing the `embassy_net_ppp::State` object.

The original implementation used:

```rust
static PPP_STATE: StaticCell<
    embassy_net_ppp::State<4, 4>
> = StaticCell::new();

let ppp_state = PPP_STATE.init_with(|| {
    embassy_net_ppp::State::<4, 4>::new()
});
```

Although `StaticCell` provides statically allocated storage for the PPP state, the firmware continued to encounter a HardFault during initialization.

The investigation narrowed the failure to the construction of the PPP state rather than UART initialization or PPP negotiation.

The STM32F410RB has 32 KiB of SRAM. The original investigation recorded a PPP state size of 12,136 bytes for `State<4, 4>`, indicating substantial memory consumption relative to the available SRAM.

The failure raised concerns about the combined RAM requirements of the PPP state, network stack, UART buffers, executor, and stack, as well as possible temporary stack usage during construction.

## 2. Investigation and Root Cause Analysis

The following findings were established during the earlier investigation:

1. **PPP state memory consumption:** The original `State<4, 4>` configuration was relatively large for the STM32F410RB's 32 KiB SRAM.

2. **Static allocation was already in use:** The PPP state was stored in a `StaticCell`. Therefore, the investigation did not identify `StaticCell` itself as the problem.

3. **Possible stack pressure:** Constructing a large value can involve temporary storage depending on the implementation and generated code. This could have contributed to the HardFault despite the final object residing in static storage.

4. **Linker configuration:** The firmware used `flip-link`, which rearranges RAM to protect static memory from stack overflow. Inspection of the linker symbols and vector table did not establish that the linker configuration was incorrect.

5. **Diagnostic limitation:** The earlier size diagnostic continued measuring `State<4, 4>` after the actual state had been reduced to `State<3, 3>`.&#x20;

**Root cause assessment:** The evidence suggests memory pressure associated with the original PPP state configuration, potentially involving stack usage during construction. 

## 3. Solution Implemented

The initial correction reduced the PPP state parameters from `State<4, 4>` to `State<3, 3>`.

The current repository implementation has subsequently adopted the smaller `State<1, 1>` configuration.

The present implementation in `stm32-f410-ppp-client/src/main.rs` is:

```rust
static PPP_STATE: StaticCell<
    embassy_net_ppp::State<1, 1>
> = StaticCell::new();
```

Initialization is performed using:

```rust
let ppp_state = PPP_STATE.init(
    embassy_net_ppp::State::<1, 1>::new()
);

let (ppp_device, ppp_runner) =
    embassy_net_ppp::new(ppp_state);
```

The current implementation retains static allocation while using the reduced PPP state configuration.

The surrounding memory configuration is also defined explicitly:

```rust
static STACK_RESOURCES:
    StaticCell<StackResources<3>> =
    StaticCell::new();

static UART_TX_BUFFER:
    StaticCell<[u8; 512]> =
    StaticCell::new();

static UART_RX_BUFFER:
    StaticCell<[u8; 512]> =
    StaticCell::new();
```

The current USART1 configuration uses PA10 as RX, PA9 as TX, and a baud rate of 115200.

The linker configuration continues to use `flip-link`, with 128 KiB of Flash and 32 KiB of RAM.

**Important:** The final implementation uses `StaticCell::init()` rather than the earlier `init_with()` approach.&#x20;

## 4. Verification and Results

The reduced PPP state configuration allowed the firmware to progress beyond the original initialization failure.

The current integrated ESP32-C6 and STM32F410RB implementation has also been successfully built, flashed, and exercised on physical hardware.

The repository's documented runtime output demonstrates the following:

- STM32F410RB firmware starts successfully.
- PPP client initialization completes.
- The PPP link progresses from `Establish` to `Network` and then `Open`.
- The PPP IPv4 configuration callback executes successfully.
- The network stack reports `link_up = true`.
- IPv4 configuration becomes available to the application.
- DNS resolution and the HTTP TCP diagnostic execute successfully.
- The MQTT client connects to the public broker and receives a QoS 1 publish acknowledgement.

These results establish that the current firmware can initialize the PPP state, start the network stack, establish the PPP link, and execute application-level network diagnostics.

## 5. Final Resolution

The original HardFault was isolated to initialization of the comparatively large `embassy_net_ppp::State<4, 4>` object in the earlier testing firmware.

Reducing the PPP state configuration addressed the immediate startup failure. The current repository uses `State<1, 1>`, retains static storage through `StaticCell`, and successfully progresses through PPP initialization and integrated hardware testing.

The evidence supports memory pressure as the likely cause of the original failure, although the precise mechanism was not conclusively established.

**Final status: RESOLVED in the current implementation.**

The historical memory-related failure is considered addressed. Further memory profiling, including measurement of the current `State<1, 1>` size and stack high-water usage, would be useful before increasing PPP state parameters or expanding the network application.
