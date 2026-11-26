## ADDED Requirements

### Requirement: Shared Tuse screenshot
The docs site SHALL use the same screenshot asset as the repository README.

#### Scenario: Building the screenshot into the site
- **WHEN** `make site` builds the README page
- **THEN** the screenshot it references is copied to the corresponding path inside `target/book`
