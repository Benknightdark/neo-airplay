/**
 * @file 前端應用層匯出進入點，提供輸出埠契約與使用案例工廠函式。
 */
export type { DependencyGateway } from "./ports/dependency-gateway";
export type { ReceiverGateway } from "./ports/receiver-gateway";
export {
  createDependencyUseCases,
  type DependencyUseCases,
} from "./use-cases/dependency-use-cases";
export {
  createReceiverUseCases,
  type ReceiverUseCases,
} from "./use-cases/receiver-use-cases";
