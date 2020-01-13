## Postcode to address API
This is a web API that fetches data from [openaddresses](https://openaddresses.io/) and provides postcode verification.  
It can be useful to verify user input, or pre-fill information (for example, the delivery address in a shopping cart).

Currently the data is only available for the Netherlands but the API could be extended to support multiple of the countries available on [openaddresses](https://openaddresses.io/).

##### Example requests
`GET /addresses?postcode=1011PN`  
`GET /addresses?postcode=1011PN&number=1`

If the postcode is valid, you will get back the list of addresses associated to it.
```json
[
    {
       "id":"5573a555-2ba2-4247-bf76-c2977d47fe8e",
       "lat":52.367645263671875,
       "lon":4.900165557861328,
       "number":"1",
       "street":"Amstel",
       "city":"Amsterdam",
       "region":"Noord-Holland",
       "postcode":"1011PN"
    }
 ]
```

##### Query parameters
- `postcode` must be a valid postcode (check https://en.wikipedia.org/wiki/Postal_codes_in_the_Netherlands).
- `number` is optional. When not specified, all the addresses associated with the postcode will be returned.

### Technologies
- [Actix web 2.0](https://github.com/actix/actix-web)
- [Diesel](https://github.com/diesel-rs/diesel)
- [Postgres](https://www.postgresql.org/)

You can run the service by using Docker, or locally (with Rust >= 1.40 and a Postgres instance).  
The first start will take a couple of minutes, as the service needs to fetch millions of address records.
