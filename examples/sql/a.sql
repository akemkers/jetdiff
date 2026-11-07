-- Accounts and their transfers
CREATE TABLE account (
    id INT PRIMARY KEY,
    owner VARCHAR(100) NOT NULL,
    balance DECIMAL(12, 2) DEFAULT 0
);

CREATE TABLE transfer (
    id INT PRIMARY KEY,
    from_account INT REFERENCES account (id),
    to_account INT REFERENCES account (id),
    amount DECIMAL(12, 2) NOT NULL
);

SELECT a.owner, sum(t.amount) AS sent
FROM account a
JOIN transfer t ON t.from_account = a.id
GROUP BY a.owner
ORDER BY sent DESC;
