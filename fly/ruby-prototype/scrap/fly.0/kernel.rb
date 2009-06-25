
  class Object
    attr_accessor :parent
  end

  class Hash
    attr_accessor :parent

    def lookup( complex_key )
      return self[key] if key?( complex_key )
      prt, hit = self, nil
      while prt and not hit
        prt = prt.parent
        hit = prt.lookup( complex_key )
      end
      return hit
    end
  end


  class Part

    def word( name, face=[], &block )
      name = name.to_s
      node = Word.new( name, face, &block )
      words[name][face] = node
      faces[name] << face
      faces[name].sort! { |a,b| a.size <=> b.size }
    end

    def initialize
      @words = Hash.new { |hash, key| hash[key] = {} }
      @faces = Hash.new { |hash, key| hash[key] = [] }
    end

  end


  Kernel = Part.new

  Kernel.instance_eval do

    ### STACK OPERATIONS ###

    # get top of stack
    word :top do
      @stack.first
    end

    # remove top of stack (returns nil if empty)
    word :drop do
      @stack.shift
    end

    # remove top of stack (returns nil if empty)
    #--
    # NOTE could there be location objects (eg. @5) that regular drop
    #       could see is on the stack and use instead?
    #++
    word :drop_at do
      (i -- drop_at)
      i = pop;
      check i;
      @stack.delete_at( i )
    end

    # drop all elements from index on
    word :drop_from, 'i' do
      i = pop;
      #check i;
      @stack.slice!(0,i)
    end

    # remove top of stack (raises error if empty)
    #pop: !!ruby
    #  check
    #  @stack.shift

    # empty stack
    word :clear do
      @stack.clear
    end

    # duplicate the top of the stack
    word :dup do
      #return unary do |num|
      #  push( num )
      #  num
      #end
      check;
      push @stack.first
    end

    # swap top two elements from stack
    word :swap do
      #return binary do |l, r|
      #  push( r )
      #  l
      #end
      #r, l = @stack.slice!(0,2)
      check 2;
      @stack[0], stack[1] = stack[1], stack[0]
    end

    # dup second item
    word :over do
      push @stack[1].dup
    end

    # arbitrary stack position swap (up)
    word :roll do
      push drop_at( i )
    end

    # arbitrary stack position swap (down)
    word :rolld do
      i = pop;
      o = pop;
      check i;
      @stack.insert(i, o)
    end

    ### BASIC MATH ###

    word '+', 'n n' do
      drop + drop
    end

    word '*', 'n n' do
      drop * drop
    end

    word '-', 'n n' do
      a,b = drop,drop; b - a
    end

    word '%', 'n n' do
      a,b = drop,drop; b % a
    end

    word '**', 'n n' do
      a,b = drop,drop; b ** a
    end

    word '/', 'n n' do
      a,b = drop,drop;
      # defeat Ruby's integer division
      if a.integer? and b.integer? and b % a != 0;
        b / a.to_f;
      else
        b / a;
      end
    end

#     word :floor, 'n' do push drop.floor         end
#     word :ceil,  'n' do push drop.ceil          end
#     word :round, 'n' do push drop.round         end
# 
#     word :abs,   'n' do push drop.abs           end
#     word :sqrt,  'n' do push Math.sqrt( drop )  end
# 
#     word :exp,   'n' do push Math.exp( drop )   end
#     word :log,   'n' do push Math.log( drop )   end
#     word :log10, 'n' do push Math.log10( drop ) end
# 
#     word :sin,   'n' do push Math.sin( drop )   end
#     word :cos,   'n' do push Math.cos( drop )   end
#     word :tan,   'n' do push Math.tan( drop )   end
#     word :sinh,  'n' do push Math.sinh( drop )  end
#     word :cosh,  'n' do push Math.cosh( drop )  end
#     word :tanh,  'n' do push Math.tanh( drop )  end
#     word :asin,  'n' do push Math.asin( drop )  end
#     word :acos,  'n' do push Math.acos( drop )  end
#     word :atan,  'n' do push Math.atan( drop )  end
#     word :sinh,  'n' do push Math.asinh( drop ) end
#     word :acosh, 'n' do push Math.acosh( drop ) end
#     word :atanh, 'n' do push Math.atanh( drop ) end
# 
#     word :atan2,  'n n' do
#       a,b = drop,drop;
#       Math.atan2( b, a )
#     end

  end

end
