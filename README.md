## Postcode to address API
This is a web API that fetches data from [openaddresses](https://openaddresses.io/) provide postcode verification.

Currently the data is only available for the Netherlands but the API could be extended to support multiple of the countries available on [openaddresses](https://openaddresses.io/).

##### Example requests
`GET /addresses?postcode=1011PN`  
`GET /addresses?postcode=1011PN&number=1`

##### Query parameters
- `postcode` must be a valid postcode (check https://en.wikipedia.org/wiki/Postal_codes_in_the_Netherlands)
- `number` is optional. When not specified, all the addresses associated with the postcode will be returned.

### Technologies
- [Actix web 2.0](https://github.com/actix/actix-web)
- [Diesel](https://github.com/diesel-rs/diesel)
- [Postgres](https://www.postgresql.org/)

You can run the service by using Docker, or locally (with Rust >= 1.40 and a Postgres instance)
