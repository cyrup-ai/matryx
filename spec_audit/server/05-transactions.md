# Transactions

The transfer of EDUs and PDUs between homeservers is performed by an exchange of Transaction messages, which are encoded as JSON objects, passed over an HTTP PUT request. A Transaction is meaningful only to the pair of homeservers that exchanged it; they are not globally-meaningful.

Transactions are limited in size; they can have at most 50 PDUs and 100 EDUs.

