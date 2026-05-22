config = {
    --!Server configuration root
    --[[@TABLE = {
        allow_edit = false,
        schema = {
            { name = "key", type = "string", label = "Config key" },
            { name = "value", type = "string", label = "Config value" },
        }
    }]]
    server = {
        --!Basic server info
        identifier = "srv_01",
        --!Server region
        --@ENUM = { "us-east", "eu-west", "ap-southeast" }
        region = "us-east",
        --!Max player count
        --@RANGE = { min = 1, max = 256 }
        max_players = 64,
        --!Features enabled
        features = {
            --!Anti-cheat system
            anti_cheat = true,
            --!Voice chat
            voice = false,
            --!Cross-play settings
            crossplay = {
                --@ENUM = { "off", "playstation", "xbox", "all" }
                platform = "off",
                --!Region lock
                region_lock = true,
                --!Allowed regions
                --@ENUM = { "us", "eu", "asia", "oceania" }
                allowed_regions = "us",
            },
        },
        --!Admin settings
        admins = {
            --!Root admin
            root = {
                name = "Admin",
                --@ENUM = { "owner", "super_admin", "admin", "mod" }
                role = "owner",
                permissions = {
                    kick = true,
                    ban = true,
                    --@RANGE = { min = 1, max = 365 }
                    ban_duration = 30,
                    --!Whisper mode
                    --@ENUM = { "all", "admins", "none" }
                    whisper = "all",
                },
            },
            --!Moderator list
            --[[@TABLE = {
                  allow_add = true,
                  allow_delete = true,
                  schema = {
                    { name = "id", type = "key", label = "User ID" },
                    { name = "name", type = "string", label = "Display name" },
                    { name = "level", type = "enum", label = "Access level", values = { "trial", "full", "senior" } },
                    { name = "active", type = "boolean", label = "Currently active" },
                    { name = "max_ban", type = "number", label = "Max ban days" },
                  }
              }]]
            moderators = {
                mod_01 = {
                    name = "Alice",
                    level = "senior",
                    active = true,
                    max_ban = 14,
                },
                mod_02 = {
                    name = "Bob",
                    level = "trial",
                    active = false,
                    max_ban = 3,
                },
                mod_03 = {
                    name = "Charlie",
                    level = "full",
                    active = true,
                    max_ban = 7,
                },
            },
        },
    },

    --!Economy system
    --[[@TABLE = {
        allow_add = true,
        allow_delete = true,
        schema = {
            { name = "id", type = "key", label = "Item ID" },
            { name = "name", type = "string", label = "Item name" },
            { name = "category", type = "enum", label = "Category", values = { "weapon", "food", "vehicle", "clothing" } },
            { name = "price", type = "number", label = "Price" },
            { name = "stackable", type = "boolean", label = "Can stack" },
            { name = "sellable", type = "boolean", label = "Can sell" },
            { name = "rarity", type = "enum", label = "Rarity", values = { "common", "uncommon", "rare", "legendary" } },
        }
    }]]
    economy = {
        --!Currency settings
        currency = {
            --@ENUM = { "USD", "EUR", "GBP", "JPY" }
            type = "USD",
            --@RANGE = { min = 0.01, max = 10000 }
            tax_rate = 5.0,
            --!Decimal places
            --@RANGE = { min = 0, max = 4 }
            decimals = 2,
            symbol = "$",
        },
        --!Shop items
        items = {
            --!Weapon category
            pistol = {
                name = "Pistol",
                category = "weapon",
                price = 500,
                stackable = false,
                sellable = true,
                rarity = "common",
                --!Damage config
                damage = {
                    --@RANGE = { min = 1, max = 100 }
                    base = 25,
                    --@RANGE = { min = 0, max = 50 }
                    falloff = 10,
                    --@ENUM = { "hitscan", "projectile", "melee" }
                    type = "hitscan",
                },
            },
            --!Food
            apple = {
                name = "Apple",
                category = "food",
                price = 5,
                stackable = true,
                sellable = false,
                rarity = "common",
                effects = {
                    --@RANGE = { min = 0, max = 100 }
                    health = 10,
                    --@RANGE = { min = 0, max = 100 }
                    hunger = 25,
                    --@ENUM = { "instant", "over_time" }
                    heal_type = "instant",
                },
            },
            --!Rare vehicle
            sports_car = {
                name = "Sports Car",
                category = "vehicle",
                price = 50000,
                stackable = false,
                sellable = true,
                rarity = "legendary",
                --!Vehicle stats
                stats = {
                    --@RANGE = { min = 0, max = 500 }
                    speed = 320,
                    --@RANGE = { min = 0, max = 100 }
                    handling = 85,
                    --@RANGE = { min = 0, max = 100 }
                    acceleration = 92,
                    --@ENUM = { "compact", "sedan", "suv", "sport", "super", "motorcycle" }
                    class = "super",
                },
                --!Spawn position
                --@MAP = true
                spawn_pos = vector3(250.5, 1800.0, 32.0),
            },
        },
    },

    --!Weather system
    weather = {
        --!Current weather preset
        --@ENUM = { "clear", "clouds", "rain", "thunder", "fog", "snow" }
        preset = "clear",
        --!Time cycle
        time = {
            --@RANGE = { min = 0, max = 24 }
            hour = 12,
            --@RANGE = { min = 0, max = 60 }
            minute = 0,
            --Dynamic time
            dynamic = true,
            --!Time speed multiplier
            --@RANGE = { min = 1, max = 100 }
            speed = 5,
        },
        --!Wind settings
        wind = {
            --@RANGE = { min = 0, max = 100 }
            speed = 15,
            --@ENUM = { "north", "south", "east", "west", "northwest", "northeast", "southwest", "southeast" }
            direction = "northwest",
            --!Gust settings
            gusts = {
                enabled = true,
                --@RANGE = { min = 1, max = 50 }
                intensity = 20,
                --@RANGE = { min = 0.5, max = 10 }
                interval = 3.5,
            },
        },
        --!Temperature zones
        --[[@TABLE = {
              allow_add = true,
              allow_delete = false,
              schema = {
                { name = "zone", type = "key", label = "Zone name" },
                { name = "base_temp", type = "number", label = "Base temperature" },
                { name = "biome", type = "enum", label = "Biome type", values = { "desert", "forest", "tundra", "ocean", "urban" } },
                { name = "humidity", type = "number", label = "Humidity percentage" },
              }
          }]]
        zones = {
            city = {
                base_temp = 25.0,
                biome = "urban",
                humidity = 60.0,
            },
            forest = {
                base_temp = 22.0,
                biome = "forest",
                humidity = 80.0,
            },
        },
    },

    --!Discord integration
    discord = {
        --!Bot token webhook
        --[[@CFX_FUNCTION = {
              args_schema = {
                { name = "message", type = "string", label = "Message content", required = true },
                { name = "channel_id", type = "string", label = "Target channel", required = true },
              }
          }]]
        send_message = function(msg, channel)
            exports.discord:send(channel, msg)
        end,
        --!Log webhook
        --[[@CFX_FUNCTION = {
              args_schema = {
                { name = "event", type = "string", label = "Event name", required = true },
                { name = "data", type = "string", label = "JSON payload", required = false },
                { name = "severity", type = "string", label = "Log level", required = false },
              }
          }]]
        log_event = function(event, data, severity)
            exports.discord:log(event, data, severity)
        end,
    },

    --!Player defaults
    player_defaults = {
        --!Spawn location
        --@MAP = true
        spawn = vector3(-150.0, 50.0, 28.0),
        --!Starting money
        --@RANGE = { min = 0, max = 100000 }
        start_money = 1000,
        --!Starting items
        --[[@TABLE = {
              allow_add = true,
              allow_delete = true,
              allow_edit = true,
              schema = {
                { name = "item", type = "key", label = "Item ID" },
                { name = "quantity", type = "number", label = "Quantity" },
              }
          }]]
        start_items = {
            water_bottle = { quantity = 2 },
            bread = { quantity = 1 },
        },
        --!Max inventory slots
        --@RANGE = { min = 10, max = 200 }
        max_inventory = 50,
        --!Respawn settings
        respawn = {
            --@RANGE = { min = 0, max = 60 }
            time = 5,
            --!Respawn location
            --@MAP = true
            location = vector2(0.0, 0.0),
            --@ENUM = { "hospital", "last_position", "random" }
            type = "hospital",
        },
    },
}
