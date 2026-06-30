use crate::DemoScenario;

const BASIC: &str = include_str!("../fixtures/demo/basic/evidence.json");
const CHECKOUT_INCIDENT: &str = include_str!("../fixtures/demo/checkout-incident/evidence.json");
const RELEASE_REGRESSION: &str = include_str!("../fixtures/demo/release-regression/evidence.json");
const WORKFLOW_FAILURE: &str = include_str!("../fixtures/demo/workflow-failure/evidence.json");
const MULTI_SOURCE_RELEASE: &str =
    include_str!("../fixtures/demo/multi-source-release/evidence.json");

pub(crate) fn packaged_demo_fixture(scenario: DemoScenario) -> &'static str {
    match scenario {
        DemoScenario::Basic => BASIC,
        DemoScenario::CheckoutIncident => CHECKOUT_INCIDENT,
        DemoScenario::ReleaseRegression => RELEASE_REGRESSION,
        DemoScenario::WorkflowFailure => WORKFLOW_FAILURE,
        DemoScenario::MultiSourceRelease => MULTI_SOURCE_RELEASE,
    }
}
