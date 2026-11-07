-- Accounts and their transfers
CREATE TABLE account (
    id INT PRIMARY KEY,
    owner VARCHAR(200) NOT NULL,
    currency CHAR(3) DEFAULT 'NOK',
    balance DECIMAL(12, 2) DEFAULT 0
);

CREATE TABLE transfer (
    id INT PRIMARY KEY,
    from_account INT REFERENCES account (id),
    to_account INT REFERENCES account (id),
    amount DECIMAL(12, 2) NOT NULL CHECK (amount > 0),
    /* when the bank booked it */
    booked_at TIMESTAMP
);

SELECT a.owner, a.currency, sum(t.amount) AS sent
FROM account a
LEFT JOIN transfer t ON t.from_account = a.id
WHERE t.booked_at IS NOT NULL
GROUP BY a.owner, a.currency
ORDER BY sent DESC
LIMIT 10;
