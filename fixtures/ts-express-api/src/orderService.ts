import express from 'express';

const router = express.Router();

// ─────────────────────────────────────────────────────────────
//  Data Types
// ─────────────────────────────────────────────────────────────

export interface Order {
  id: string;
  userId: string;
  items: OrderItem[];
  status: 'pending' | 'confirmed' | 'shipped' | 'cancelled';
  total: number;
  createdAt: Date;
}

export interface OrderItem {
  productId: string;
  quantity: number;
  price: number;
}

export interface User {
  id: string;
  email: string;
  name: string;
  isActive: boolean;
  tier: 'standard' | 'premium';
}

// ─────────────────────────────────────────────────────────────
//  Simulated DB layer (in-memory for fixture)
// ─────────────────────────────────────────────────────────────

const usersDb: Map<string, User> = new Map([
  ['u1', { id: 'u1', email: 'alice@example.com', name: 'Alice', isActive: true, tier: 'premium' }],
  ['u2', { id: 'u2', email: 'bob@example.com', name: 'Bob', isActive: true, tier: 'standard' }],
  ['u3', { id: 'u3', email: 'carol@example.com', name: 'Carol', isActive: false, tier: 'standard' }],
]);

const ordersDb: Map<string, Order> = new Map([
  ['o1', { id: 'o1', userId: 'u1', items: [{ productId: 'p1', quantity: 2, price: 50 }], status: 'confirmed', total: 100, createdAt: new Date('2024-01-01') }],
  ['o2', { id: 'o2', userId: 'u2', items: [{ productId: 'p2', quantity: 1, price: 200 }], status: 'pending', total: 200, createdAt: new Date('2024-01-02') }],
  ['o3', { id: 'o3', userId: 'u1', items: [{ productId: 'p3', quantity: 3, price: 30 }], status: 'shipped', total: 90, createdAt: new Date('2024-01-03') }],
]);

// Simulated async DB calls with artificial latency
async function dbGetUser(id: string): Promise<User | null> {
  await new Promise(r => setTimeout(r, 10)); // 10ms simulated DB latency
  return usersDb.get(id) ?? null;
}

async function dbGetOrder(id: string): Promise<Order | null> {
  await new Promise(r => setTimeout(r, 10));
  return ordersDb.get(id) ?? null;
}

async function dbGetAllOrders(): Promise<Order[]> {
  await new Promise(r => setTimeout(r, 20));
  return Array.from(ordersDb.values());
}

async function dbGetUsersBatch(ids: string[]): Promise<Map<string, User>> {
  await new Promise(r => setTimeout(r, 15)); // Single batch query
  const result = new Map<string, User>();
  for (const id of ids) {
    const user = usersDb.get(id);
    if (user) result.set(id, user);
  }
  return result;
}

// ─────────────────────────────────────────────────────────────
//  🐛 BUG #1: N+1 Query — processOrdersWithUserInfo
//
//  This function fetches all orders, then for EACH order makes
//  a separate DB call to get the user. With 100 orders, this
//  creates 101 DB queries instead of 2.
//
//  CHRONOS should detect:
//  - High invocation count on `dbGetUser` per session
//  - `dbGetUser` called inside loop (hot path detected)
//  - Mutation tests should catch boundary conditions
// ─────────────────────────────────────────────────────────────

export async function processOrdersWithUserInfo(): Promise<Array<Order & { userName: string }>> {
  const orders = await dbGetAllOrders();
  const result: Array<Order & { userName: string }> = [];

  // 🔴 N+1 BUG: separate DB call per order
  for (const order of orders) {
    const user = await dbGetUser(order.userId);  // BUG: should batch fetch
    if (user && user.isActive) {
      result.push({ ...order, userName: user.name });
    }
  }

  return result;
}

// ─────────────────────────────────────────────────────────────
//  ✅ FIXED VERSION: processOrdersWithUserInfoOptimized
//
//  Batch-fetches all users in ONE query, then joins in memory.
//  With 100 orders, this is always exactly 2 DB queries.
// ─────────────────────────────────────────────────────────────

export async function processOrdersWithUserInfoOptimized(): Promise<Array<Order & { userName: string }>> {
  const orders = await dbGetAllOrders();

  // ✅ FIXED: Collect all unique userIds first
  const userIds = [...new Set(orders.map(o => o.userId))];
  const usersMap = await dbGetUsersBatch(userIds);  // single batch query

  return orders
    .filter(order => usersMap.get(order.userId)?.isActive)
    .map(order => ({
      ...order,
      userName: usersMap.get(order.userId)!.name,
    }));
}

// ─────────────────────────────────────────────────────────────
//  🐛 BUG #2: Off-by-one in discount calculation
//
//  Premium users should get 10% discount on orders > 100.
//  Bug: uses `>= 100` instead of `> 100` — gives discount at
//  exactly 100 when they shouldn't.
// ─────────────────────────────────────────────────────────────

export function calculateDiscount(order: Order, user: User): number {
  if (user.tier !== 'premium') return 0;

  // 🔴 BUG: should be `> 100` not `>= 100`
  if (order.total >= 100) {
    return order.total * 0.10;
  }
  return 0;
}

// ─────────────────────────────────────────────────────────────
//  🐛 BUG #3: Missing null check — getOrderWithUser
//
//  Does not handle the case where user is null (deleted user).
//  Will throw TypeError in production when a user is removed.
// ─────────────────────────────────────────────────────────────

export async function getOrderWithUser(orderId: string): Promise<{ order: Order; user: User } | null> {
  const order = await dbGetOrder(orderId);
  if (!order) return null;

  const user = await dbGetUser(order.userId);
  // 🔴 BUG: no null check on user — will crash if user deleted
  return { order, user: user! };
}

// ─────────────────────────────────────────────────────────────
//  Validation utilities (correct implementations)
// ─────────────────────────────────────────────────────────────

export function validateOrderStatus(
  current: Order['status'],
  next: Order['status']
): boolean {
  const validTransitions: Record<Order['status'], Order['status'][]> = {
    pending: ['confirmed', 'cancelled'],
    confirmed: ['shipped', 'cancelled'],
    shipped: [],
    cancelled: [],
  };
  return validTransitions[current].includes(next);
}

export function calculateOrderTotal(items: OrderItem[]): number {
  return items.reduce((sum, item) => sum + item.quantity * item.price, 0);
}

// ─────────────────────────────────────────────────────────────
//  Express API Routes
// ─────────────────────────────────────────────────────────────

router.get('/orders', async (req, res) => {
  try {
    const orders = await processOrdersWithUserInfo();
    res.json(orders);
  } catch (err) {
    res.status(500).json({ error: 'Internal server error' });
  }
});

router.get('/orders/:id', async (req, res) => {
  try {
    const result = await getOrderWithUser(req.params.id);
    if (!result) return res.status(404).json({ error: 'Order not found' });
    res.json(result);
  } catch (err) {
    res.status(500).json({ error: 'Internal server error' });
  }
});

router.post('/orders/:id/discount', async (req, res) => {
  try {
    const result = await getOrderWithUser(req.params.id);
    if (!result) return res.status(404).json({ error: 'Order not found' });
    const discount = calculateDiscount(result.order, result.user);
    res.json({ orderId: req.params.id, discount });
  } catch (err) {
    res.status(500).json({ error: 'Internal server error' });
  }
});

export default router;
