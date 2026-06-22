class OrgDatabase {
  private collections = new Map<string, any>();

  constructor(public org: string) {}

  getCollection<T>(name: string, factory: (org: string) => T): T {
    if (!this.collections.has(name)) {
      this.collections.set(name, factory(this.org));
    }
    return this.collections.get(name) as T;
  }

  clearCollection(name: string) {
    this.collections.delete(name);
  }
}

const orgDatabases = new Map<string, OrgDatabase>();

export function getOrgDb(org: string): OrgDatabase {
  if (!orgDatabases.has(org)) {
    orgDatabases.set(org, new OrgDatabase(org));
  }
  return orgDatabases.get(org)!;
}
