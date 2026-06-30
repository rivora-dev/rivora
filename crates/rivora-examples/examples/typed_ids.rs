use rivora_core::{
    AbilityVersion, ObservationId, OrganizationId, ReceiptId, SchemaVersion, ServiceId,
};

fn main() {
    let obs = ObservationId::new("obs_001").expect("valid observation id");
    let svc = ServiceId::new("svc-api").expect("valid service id");
    let receipt = ReceiptId::new("receipt_42").expect("valid receipt id");
    let org = OrganizationId::new("org-acme").expect("valid organization id");

    println!("observation value: {}", obs);
    println!("observation debug: {:?}", obs);
    println!("observation kinded: {}", obs.to_kinded_string());

    println!("service value: {}", svc);
    println!("service debug: {:?}", svc);
    println!("service kinded: {}", svc.to_kinded_string());

    println!("receipt value: {}", receipt);
    println!("receipt debug: {:?}", receipt);
    println!("receipt kinded: {}", receipt.to_kinded_string());

    println!("organization value: {}", org);
    println!("organization debug: {:?}", org);
    println!("organization kinded: {}", org.to_kinded_string());

    match ObservationId::new("") {
        Ok(_) => println!("unexpected: empty observation id was accepted"),
        Err(e) => println!("rejected empty observation id: kind={}", e.kind().as_str()),
    }

    let random = ServiceId::new_random();
    println!("random service id: {}", random);
    println!("random service debug: {:?}", random);
    println!("random service kinded: {}", random.to_kinded_string());

    let schema = SchemaVersion::new(1, 0, 0);
    println!("schema version: {}", schema);
    println!("schema version debug: {:?}", schema);

    let ability = AbilityVersion::parse("0.2.1").expect("valid ability version");
    println!("ability version: {}", ability);
    println!("ability version debug: {:?}", ability);
}
