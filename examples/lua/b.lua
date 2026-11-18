--- Quotes for securities.
local M = {}

local MAX_AGE <const> = 120

--- The last price for a security.
---@param ticker string
---@param price number
---@param currency string?
function M.new(ticker, price, currency)
  return setmetatable({
    ticker = ticker,
    price = price,
    currency = currency or "NOK",
    time = os.time(),
  }, { __index = M })
end

function M:format()
  return string.format("%s: %.2f %s", self.ticker, self.price, self.currency)
end

function M:is_old(now)
  return now - self.time > MAX_AGE
end

--- Parses `TICKER PRICE [CURRENCY]`, like "EQNR 271.40 NOK".
local function parse(line)
  local ticker, price, currency = line:match("^(%w+)%s+([%d.]+)%s*(%a*)$")
  if not ticker then
    return nil, "bad line: " .. line
  end
  return M.new(ticker, tonumber(price), currency ~= "" and currency or nil)
end

M.parse = parse

return M
