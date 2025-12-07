Коротко — основні функції
- Локальний (той самий chain) крос‑аплікейшн (синхронний): ContractRuntime::call_application(...)
- Крос‑чейн передача nat токенів: ContractRuntime::transfer(...)
- Додавання контексту/атрибуції (щоб стейк‑ап знав, хто відправив): ContractRuntime::prepare_message(...).with_authentication().send_to(dest_chain)
- На приймаючій аплікації: дістати authenticated caller id через ContractRuntime::authenticated_caller_id() (або відповідний метод рантайму) і обробити повідомлення/operation в execute_operation / message handler

Що імпортувати (мінімум)
Rust


use linera_sdk::contract::{Contract, ContractRuntime, ContractAbi};
use linera_sdk::{ApplicationId, Account, AccountOwner, Amount, ChainId};
(Додатково імпортуйте ABI типи вашої цільової аплікації — її Operation/Response — наприклад use crate::staking::StakingAbi;)

1) Сценарій: виклик аплікації на тому ж chain (синхронно)
- Виклик: runtime.call_application(authenticated, application_id, &operation)
- Пояснення: generic: A: ContractAbi; якщо authenticated=true — called app отримає authenticated caller id.

Приклад:
Rust


// sender app, виклик same‑chain staking app
let staking_id: ApplicationId<staking::StakingAbi> = /* отримали або в state */;
let op = staking::Operation::AddStake { staker: owner.clone(), amount };
let resp: staking::Response = self.runtime.call_application(true, staking_id, &op);
// обробіть resp за потреби
2) Сценарій: крос‑чейн — відправити NAT токени на staking‑app на іншому chain і передати атрибуцію
- Крок 1 (transfer): runtime.transfer(source_owner, destination_account, amount)
  - destination_account.owner = ApplicationId стейк‑апа (тобто гроші "лягатимуть" на рахунок аплікації на цільовому чейні)
- Крок 2 (notify): runtime.prepare_message(payload_bytes).with_authentication().send_to(dest_chain_id)
  - payload містить, наприклад, staker id + amount — щоб стейк‑ап знав, кому зарахувати

Приклад:
Rust


// sender app, cross-chain deposit
let dest_account = Account {
    chain_id: dest_chain_id,
    owner: staking_app_id.into(), // ApplicationId -> owner
};
let source = AccountOwner::from(sender_owner.clone()); // або AccountOwner::CHAIN
self.runtime.transfer(source, dest_account, amount);

// сформувати повідомлення з attribution
let msg = StakingMessage::Deposit { staker: sender_owner.clone(), amount };
let bytes = bincode::serialize(&msg).unwrap();
self.runtime
    .prepare_message(bytes)
    .with_authentication() // опціонально: щоб отримувач бачив authenticated caller id
    .send_to(dest_chain_id);
3) Обробка на приймаючій аплікації (staking app)
- У методі execute_operation / message handler:
  - десеріалізувати повідомлення/operation
  - отримати authenticated caller id (якщо потрібно) через runtime.authenticated_caller_id() (повертає Option<ApplicationId>)
  - зарахувати баланс користувачу (записати в state)
  - опціонально: перевірити balance(application_id) чи очікуване надходження прийшло

Приклад:
Rust


// у staking app
fn execute_operation(&mut self, op: IncomingOperation) {
    match op {
        IncomingOperation::DepositMsg(bytes) => {
            let msg: StakingMessage = bincode::deserialize(&bytes).unwrap();
            // Якщо повідомлення прийшло з with_authentication(), можна дізнатись caller:
            let maybe_caller = self.runtime.authenticated_caller_id(); // Option<ApplicationId>
            // Звичайний шлях: зарахувати msg.amount користувачу msg.staker
            let prev = self.state.stakes.get(&msg.staker).unwrap_or_default();
            self.state.stakes.insert(msg.staker.clone(), prev + msg.amount);
        }
        // інші операції...
    }
}
Важливі практичні нюанси
- Attribution: просто transfer змінює баланс account=ApplicationId, але не каже, якому юзеру — тому потрібно додаткове повідомлення з контекстом (як ви писали).
- Atomicity: transfer + повідомлення — не атомарні. Може статись, що transfer пройшов, але message загублено/оброблено пізніше. Тому:
  - Реалізуйте на приймаючій стороні recovery: періодична перевірка балансу аплікації і зіставлення з уже зарахованими сумами.
  - Або вимагайте підтвердження/логіку повтору на стороні відправника.
- authenticated флаг: якщо ви хочете, щоб приймаючий app точно бачив, хто ініціював (ApplicationId як caller), використовуйте with_authentication() при відправленні повідомлення або встановлюйте authenticated=true при call_application.
- Не викликайте call_application під час finalize (це заборонено — runtime це перевіряє).
- Типи ABI: при call_application generic A повинна відповідати ABI викликаної аплікації (Operation/Response). Імпортуйте ті типи та серіалізатори, які генеруються для ABI.



Якщо ціль‑аплікація НА ІНШОМУ чейні (крос‑чейн) — не можна синхронно викликати її через call_application:
- Патерн: зробити transfer токенів на account аплікації на цільовому чейні, і паралельно відправити повідомлення з attribution (хто і скільки).
  - 1) runtime.transfer(source_owner, Account { chain_id: dest_chain, owner: staking_app_id.into() }, amount);
  - 2) let payload = serialize(Deposit { staker, amount, nonce }); 
       runtime.prepare_message(payload).with_authentication().send_to(dest_chain);
- На приймаючій аплікації — у message handler десеріалізуєш повідомлення і зараховуєш stake користувачу (self.state.stakes[staker] += amount).

Чому потрібне окреме повідомлення
- Transfer просто переміщує NAT на account = ApplicationId, але не містить даних про which user to credit. Повідомлення дає attribution (owner id, можливо nonce/tx id для ідeмпотентності).
- Transfer + message — не атомарні. Можуть бути ситуації: transfer пройшов, message загубилось/запізнюється. Тому роби обробку на приймаючій стороні з механізмом хоум‑reconciliation (перевірка балансу аплікації, idempotency, replay protection).