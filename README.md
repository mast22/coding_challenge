# Rust challenge

## Implementation

The solution uses openspec as a directive for LLM-assisted coding. 
Before the implementation has began the requirements have been broken down into 2 main parts that are to be implemented into the system:

1. Architecture, design and API interface
2. Business cases

The separation was in order to ease out the implementation context for the model. The first part was easy - to build a scaffolding for the internal system that is to be build in second part. The libraries, the primary module structure, the tests approach is established - the approximate engine implementation is designed.

Business cases were extracted during research from the pdf and specified how the system was working and were tied together in model to verify that we have everything specified.

Business cases were injected into scaffolding and the implementation was done. LLM-implemetation was done via grok workflows - graph development approach.

Post implementation review was done manually - using reviewer agent, and visual inspection. Indicating some problematic parts and using plans and manual code to fix them.

Then a simple perf-testing for 1M rows case was done, it has indicated that the solution is quite resource efficient. Yet we do not push for ultimate performance.

## Testing approach

Generally we want to separate the testing strategy into data cases, code logic and inteface.

- [Data cases](tests/integration_tests.rs)- is cases that we could encounter and different combinations of disputes, chargebacks and resolution. Is the most important testing because of covers edge cases with test data.
- Code logic - unit tests for engine
- [Inteface](tests/cli.rs) - how the bin will be called

Data cases and inteface are suitable for testing in integration testing.
Code logic is of less importance, mainly because code is a frequent subject to change and having a wide unit test suite only means that is more code to change

Data cases are stored in tests/test_data as .csv files and represent different cases.

## Safety & Robustness

Anyhow and thiserror libs were used for better error representations, no unsafe.

## Efficiency

Main points of the solution:
- Engine::apply uses mainly HashMap lookups
- Tested for 1M rows, solution stays under 100MB
- Ways to improve the solution:
    - using &str instead of owner String in parser, which we now use mainly for convenience
    - the big one: clients are independent from each other, that opens a possibility for multi-threading.
    - Look ahead of transactions would allow to see when we could drop unused clients, or drop them from memory and print out when their account is locked.
    - HashMap capacity, but that's minor


## Maintainbility

The biggest current obstacle to maintainability is underdeveloped unit test case suite. It's too big and convoluted, ideally it should be really slim, in my opinion, and the integration suite should encompass more thorugh cases.

Rest of the codebase follows ordinary SOLID/DRY/patterns practices with some Rust ideology supported by clippy, fmt.