--//Top Level Comment Here
config = {
    --//Language setting
    --!ENUM = { "jp", "en" }
    locale = "en",
    --//Enable shop
    enable_shop = true,
    --//Shop list
    shop = {
        --//Blip position
        --!MAP = true
        blip_pos = vector2(1.0, 2.0),
        --//Shop position
        --!MAP = true
        pos = vector3(10.0, 95.0, 20.0),
        --//Shop price
        --!RANGE = { min = 0, max = 10 }
        price = 10,
        --//enum
        --!ENUM = { "a", "b", "c" }
        test_enum = "a",
    },
    --//Table example
    --[[TABLE = {
  layout = "items",
  schema = {
    { name = "keyx", type = "string", is_key = true, label = "itemcode",description = "itemcode" },
    { name = "label", type = "string", label = "label",description = "label" },
    { name = "price", type = "number", label = "price",description = "price" },
    { name = "type", type = "string", label = "type",description = "type" }
  }
}]]
    items = {
        new_item_1776369615141 = {
            key = "new_item_1776369615141",
            label = "Drinking Water",
            price = 5,
            type = "food",
        },
        new_item_1776369860981 = {
            key = "new_item_1776369860981",
            label = "Drinking Water",
            price = 5,
            type = "food",
        },
    },
    --//Items list this table can edit/add/remove
    --[[ITEMS = {
  layout = "items",
  schema = {
    { name = "key", type = "string", is_key = true, label = "key" },
    { name = "label", type = "string", label = "label" },
    { name = "price", type = "number", label = "price" },
    { name = "type", type = "string", label = "type" }
  }
}]]
    items2 = { ["water_bottle"] = { key = "water_bottle", label = "Drinking Water", price = 5, type = "food" } },
    --//Discord notification
    --[[CFX_FUNCTION = { resource_name = "my_webhook", function_name = "send_hook" }]]
    webhooks = function(player_id)
        exports.my_webhook:send_hook(player_id)
    end,
    --//Event hook
    call_events = send_event(),
}
