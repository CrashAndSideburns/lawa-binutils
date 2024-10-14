widgets = {
    ram = {
        style = function(address)
            if address == emulator.program_counter then
                return { fg = { r = 0x00, g = 0x00, b = 0x00 }, bg = { r = 0xFF, g = 0xFF, b = 0xFF } }
            else
                return {}
            end
        end
    },
    registers = {
        style = function(_) return {} end,
    },
    control_status_registers = {
        style = function(_) return {} end,
    },
}
