import { Segment } from "../../components/Segment";
import { credentialGroups, inGroup } from "./groups";
import type { Credential } from "./data";
export function GroupTabs({ rows, value, onChange }: { rows: Credential[]; value: string; onChange: (id: string) => void }) {
  const options: [string, string][] = credentialGroups.flatMap(([id, label]) => {
    const count = inGroup(rows, id).length;
    return count || id === "all" ? [[id, `${label} · ${count}`]] : [];
  });
  return <div className="credential-group-scroll"><Segment label="凭证分组" className="credential-groups" value={value} onChange={onChange} options={options} /></div>;
}
