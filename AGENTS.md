# AsterGate 项目契约

AsterGate 是独立的、可自托管的通用 Identity Provider 和 Authorization Server。它提供
Account、Organization、Membership、Group、Application、API Resource、Role、Permission、
OAuth/OIDC、会话、审计和 Connector 能力。AsterDrive 是一个可选的下游客户端和资源服务器，
不是 AsterGate 的产品前提；AsterForge 是工程底座，不是 AsterGate 的领域模型。

## 设计边界

- 一个实例拥有一个 canonical issuer 和一个 identity/signing domain；Organization 是租户，
  不是 issuer 或 Realm。
- Account 是全局身份；Membership 是 Account 在 Organization 中的身份；Group 控制应用准入，
  Role/Permission 控制进入应用后的授权。
- 下游 OIDC/OAuth 通过注入式 connector/plugin 注册路由。插件只做协议适配，不创建第二套
  client、grant、authorization code、session 或 token 存储。
- Application access policy 与 resource authorization 必须分开：`allowed_groups` 决定能否进入
  Application，resource audience/scope/permission 决定进入后能做什么。
- AsterGate 公共模型不得引入 AsterDrive workspace/file/share 等产品实体，也不得把 AsterDrive
  的权限名称写死成通用 Gate 权限。

## 强类型优先

尽可能把错误状态放到类型系统和模块边界中，而不是依赖字符串约定或无验证 JSON：

- 外部输入使用按领域拆分的 DTO，DTO 只负责 Serde/OpenAPI；Service command 不依赖 Axum、
  Serde 或 HTTP 类型。
- Protocol、Connector、ClientType、Application admission、Grant、Token、Role、Permission、
  Resource 等稳定概念使用专门的 struct/enum/newtype；不要用一个 `String` 承担多个语义。
- 持久化记录和领域对象分开，Repository 只负责查询、条件更新和事务内写入，不负责产品决策。
- 用显式状态、生命周期和错误类型表达 pending/active/revoked/expired 等状态；不要把所有流程
  塞入一个未验证的 JSON `flows` 字段。
- 复用 AsterDrive 已验证的强类型建模经验，但只迁移通用边界，不复制 AsterDrive 产品语义。

强类型优先不等于给所有字符串和静态数据再包一层类型：

- 新增 struct/enum/newtype 应能阻止具体非法状态、隔离不同语义或约束生命周期；仅为改名、
  防止常量拼写错误或包装已有类型，不足以引入新的抽象。
- 配置键、静态注册元数据等优先使用常量和现有 `ConfigDefinition`；配置值的 URL、策略、
  安全模式等领域约束仍应通过类型和校验表达。避免为同一配置维护枚举、键名映射、注册列表
  和集中式元数据分支等多套同步结构。
- 要求参考 AsterDrive 时，先核对其实际定义、调用方和测试，沿用适用的成熟结构；只有明确
  的 AsterGate 领域差异才引入新设计，并说明原因，不把自行增加的抽象称为对齐 AsterDrive。

## 代码结构

```text
src/api/dto/*              HTTP wire DTO and OpenAPI schemas
src/api/routes/*           transport parsing, auth guard, response mapping
src/services/*             use-case orchestration and transaction boundaries
src/services/*/models.rs   domain commands and results
src/db/repository/*        scoped queries and conditional writes
src/connectors/*           injected protocol/provider registries
crates/aster_gate_migration product schema migrations
tests/*                    vertical flows, protocol contracts, migration and generation tests
```

新增管理 API 必须使用 `aster_forge_api_docs_macros`，并注册到 OpenAPI 汇总；变更 DTO 后必须
重新生成 `frontend-panel/generated/openapi.json` 和 `api.generated.ts`。协议响应不得套用普通
管理 API envelope。

## 验证要求

实现一个能力时至少覆盖正常路径、越权/跨 Organization、非法输入、重复/重放、事务失败和
生命周期边界。认证和授权代码、refresh rotation、group admission、resource audience、role
permission 必须有真实 router + SQLite vertical test；必要时再扩展 PostgreSQL/MySQL 矩阵。

常用检查：

```bash
cargo fmt --all
cargo test --features openapi
cargo clippy --all-targets --all-features -- -D warnings
cd frontend-panel && bun run generate-api && bun run check
```

只有代码、migration、协议行为、生成产物和测试证据同时闭环，才能把 RFC capability 标记为
Implemented。未完成能力不得出现在 discovery 或产品文档的已支持列表中。
