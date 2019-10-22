#[cfg(test)]
mod tests {
    use std::panic;

    use actix_web::{
        App,
        dev::Service,
        http::{header, StatusCode}, HttpRequest, HttpResponse, test, web,
    };
    use diesel::RunQueryDsl;

    use lazy_static::lazy_static;

    use crate::addresses;
    use crate::data::create_or_update_addresses;
    use crate::db::{init_test_connection_pool, Pool};
    use crate::models::{Address, AddressRecord};

    embed_migrations!("./migrations");

    lazy_static! {
        static ref POOL: Pool = {
            let pool = init_test_connection_pool();
            // Run migrations once for all tests
            embedded_migrations::run(&pool.get().unwrap());
            pool
        };
    }

    fn run_test<T, R>(test: T) -> R
        where T: FnOnce() -> R + panic::UnwindSafe
    {
        setup();
        let result = panic::catch_unwind(|| { test() });
        teardown();
        assert!(result.is_ok());
        result.unwrap()
    }

    fn setup() {
        use crate::schema::addresses;
        // Clear data from previous tests
        diesel::delete(addresses::table)
            .execute(&POOL.get().unwrap())
            .expect("Couldn't delete addresses table");
    }

    fn teardown () {}

    #[test]
    fn test_get_addresses_missing_query_param() {
        run_test(|| {
            let mut app = test::init_service(
                App::new()
                    .data(POOL.clone())
                    .route("/addresses", web::get().to_async(addresses))
            );

            let req = test::TestRequest::get()
                .uri("/addresses")
                .to_request();

            let resp = test::block_on(app.call(req)).unwrap();
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        })
    }

    #[test]
    fn test_get_no_addresses() {
        run_test(|| {
            let mut app = test::init_service(
                App::new()
                    .data(POOL.clone())
                    .route("/addresses", web::get().to_async(addresses))
            );

            let req = test::TestRequest::get()
                .uri("/addresses?postcode=2222AA")
                .to_request();

            let resp: Vec<Address> = test::read_response_json(&mut app, req);
            assert_eq!(resp.len(), 0);
        })
    }

    #[test]
    fn test_get_addresses_postcode_only() {
        run_test(|| {
            let mut app = test::init_service(
                App::new()
                    .data(POOL.clone())
                    .route("/addresses", web::get().to_async(addresses))
            );

            create_test_set();

            let req = test::TestRequest::get()
                .uri("/addresses?postcode=2222AA")
                .to_request();

            let resp: Vec<Address> = test::read_response_json(&mut app, req);
            assert_eq!(resp.len(), 4);
        })
    }

    #[test]
    fn test_get_addresses_postcode_and_number() {
        run_test(|| {
            let mut app = test::init_service(
                App::new()
                    .data(POOL.clone())
                    .route("/addresses", web::get().to_async(addresses))
            );

            create_test_set();

            let req = test::TestRequest::get()
                .uri("/addresses?postcode=2222AA&number=1")
                .to_request();

            let resp: Vec<Address> = test::read_response_json(&mut app, req);
            assert_eq!(resp.len(), 1);
            assert_eq!(resp[0].postcode, "2222AA");
            assert_eq!(resp[0].number, "1");

            let req = test::TestRequest::get()
                .uri("/addresses?postcode=2222AA&number=2")
                .to_request();

            let resp: Vec<Address> = test::read_response_json(&mut app, req);
            assert_eq!(resp.len(), 3);
            assert_eq!(resp[0].postcode, "2222AA");
            assert_eq!(resp[0].number, "2");
            assert_eq!(resp[1].postcode, "2222AA");
            assert_eq!(resp[1].number, "2A");
            assert_eq!(resp[2].postcode, "2222AA");
            assert_eq!(resp[2].number, "2B");

            let req = test::TestRequest::get()
                .uri("/addresses?postcode=2222AA&number=2A")
                .to_request();

            let resp: Vec<Address> = test::read_response_json(&mut app, req);
            assert_eq!(resp.len(), 1);
            assert_eq!(resp[0].postcode, "2222AA");
            assert_eq!(resp[0].number, "2A");

            let req = test::TestRequest::get()
                .uri("/addresses?postcode=2222AA&number=2B")
                .to_request();

            let resp: Vec<Address> = test::read_response_json(&mut app, req);
            assert_eq!(resp.len(), 1);
            assert_eq!(resp[0].postcode, "2222AA");
            assert_eq!(resp[0].number, "2B");
        })
    }

    fn create_test_set() {
        create_or_update_addresses(
            &POOL.get().unwrap(),
            &[
                AddressRecord {
                    lat: 2.0,
                    lon: 1.0,
                    number: "1".to_string(),
                    street: "Street".to_string(),
                    city: "City".to_string(),
                    region: "Region".to_string(),
                    postcode: "2222AA".to_string()
                },
                AddressRecord {
                    lat: 3.0,
                    lon: 2.0,
                    number: "2".to_string(),
                    street: "Street".to_string(),
                    city: "City".to_string(),
                    region: "Region".to_string(),
                    postcode: "2222AA".to_string()
                },
                AddressRecord {
                    lat: 4.0,
                    lon: 3.0,
                    number: "2A".to_string(),
                    street: "Street".to_string(),
                    city: "City".to_string(),
                    region: "Region".to_string(),
                    postcode: "2222AA".to_string()
                },
                AddressRecord {
                    lat: 5.0,
                    lon: 4.0,
                    number: "2B".to_string(),
                    street: "Street".to_string(),
                    city: "City".to_string(),
                    region: "Region".to_string(),
                    postcode: "2222AA".to_string()
                },
            ]
        );
    }
}
