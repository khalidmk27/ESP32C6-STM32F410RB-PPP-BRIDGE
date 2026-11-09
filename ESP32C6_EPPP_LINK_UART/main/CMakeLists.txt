idf_component_register(SRCS "main.c"
                    INCLUDE_DIRS "."
                    REQUIRES
                        esp_driver_uart
                        eppp_link
                        esp_event
                        esp_netif
                        esp_wifi
                        nvs_flash
                    )