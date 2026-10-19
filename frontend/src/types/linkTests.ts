/**
 * Contrato de dados do teste de enlace (ADR 015 §5.1). Os tipos vêm dos
 * bindings que o backend exporta por `ts-rs`; este módulo só os reúne num
 * ponto de importação e acrescenta apelidos derivados que a tela usa.
 */

export type { LinkTest } from '@/bindings/LinkTest'
export type { LinkTestDirection } from '@/bindings/LinkTestDirection'
export type { LinkTestEndpoint } from '@/bindings/LinkTestEndpoint'
export type { LinkTestFix } from '@/bindings/LinkTestFix'
export type { LinkTestFlow } from '@/bindings/LinkTestFlow'
export type { LinkTestKind } from '@/bindings/LinkTestKind'
export type { LinkTestLeg } from '@/bindings/LinkTestLeg'
export type { LinkTestPage } from '@/bindings/LinkTestPage'
export type { LinkTestPlan } from '@/bindings/LinkTestPlan'
export type { LinkTestPlanLeg } from '@/bindings/LinkTestPlanLeg'
export type { LinkTestRequest } from '@/bindings/LinkTestRequest'
export type { LinkTestRole } from '@/bindings/LinkTestRole'
export type { LinkTestSample } from '@/bindings/LinkTestSample'
export type { LinkTestSampleEvent } from '@/bindings/LinkTestSampleEvent'
export type { LinkTestSchedule } from '@/bindings/LinkTestSchedule'
export type { LinkTestScheduleInput } from '@/bindings/LinkTestScheduleInput'
export type { LinkTestSpec } from '@/bindings/LinkTestSpec'
export type { LinkTestSpecInput } from '@/bindings/LinkTestSpecInput'
export type { LinkTestStage } from '@/bindings/LinkTestStage'
export type { LinkTestStageKey } from '@/bindings/LinkTestStageKey'
export type { LinkTestStatus } from '@/bindings/LinkTestStatus'
export type { LinkTestStepStatus } from '@/bindings/LinkTestStepStatus'
export type { LinkTestSummary } from '@/bindings/LinkTestSummary'

import type { LinkTestFix } from '@/bindings/LinkTestFix'
import type { ThroughputEngine } from '@/bindings/ThroughputEngine'

/** Motor de uma perna (`iperf3` ou `btest`). */
export type LinkTestEngine = ThroughputEngine

/** O passo "instalar o pacote" do plano (o `check` do plugin achou faltando). */
export type LinkTestInstallPackageFix = Extract<LinkTestFix, { kind: 'install_package' }>
