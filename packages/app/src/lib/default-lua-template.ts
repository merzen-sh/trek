export const DEFAULT_LUA_TEMPLATE = `config = {
    --!Server configuration root
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
            anti_cheat = true,
            voice = true,
        },
    },
    --!Economy system
    economy = {
        --[[@TABLE = {
            allow_add = true,
            allow_delete = true,
            allow_edit = true,
            schema = {
                { name = "id", type = "key", label = "Item ID" },
                { name = "name", type = "string", label = "Item name" },
                { name = "price", type = "number", label = "Price" },
            }
        }]]
        items = {
            pistol = {
                id = "pistol",
                name = "Pistol",
                price = 500,
            },
        },
    },
    --!Player defaults
    player_defaults = {
        spawn = vector3(-150.0, 50.0, 28.0),
        start_money = 1000,
    },
}`;
