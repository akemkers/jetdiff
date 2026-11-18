--- Quotes for securities.
local M = {}

local MAX_AGE <const> = 60

--- The last price for a security.
---@param ticker string
---@param price number
function M.new(ticker, price)
  return setmetatable({ ticker = ticker, price = price, time = os.time() }, { __index = M })
end

function M:format()
  return string.format("%s: %.2f", self.ticker, self.price)
end

function M:is_old(now)
  return now - self.time > MAX_AGE
end

local function parse(line)
  local ticker, price = line:match("^(%w+)%s+([%d.]+)$")
  if not ticker then
    error("bad line: " .. line)
  end
  return M.new(ticker, tonumber(price))
end

M.parse = parse

return M
